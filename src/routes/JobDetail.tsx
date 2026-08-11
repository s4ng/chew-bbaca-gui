import { useEffect, useRef, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

import { formatDuration, formatTime, MODULE_LABEL, STATUS_LABEL } from "../lib/format";
import { jobsCancel, jobsGet, jobsLog, onLog, onProgress, onState, reportOpen } from "../lib/ipc";
import { asAppError, type Job, type LogStream } from "../lib/types";

interface Line {
  stream: LogStream;
  text: string;
}

/** 화면에 유지할 최대 로그 줄 수. 전체 기록은 항상 로그 **파일**에 남는다. */
const MAX_LINES = 4000;

export default function JobDetail({
  jobId,
  onBack,
  onChanged,
}: {
  jobId: string;
  onBack: () => void;
  onChanged: () => Promise<void> | void;
}) {
  const [job, setJob] = useState<Job | null>(null);
  const [lines, setLines] = useState<Line[]>([]);
  const [progress, setProgress] = useState<{ fraction: number; label: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [follow, setFollow] = useState(true);
  const logRef = useRef<HTMLDivElement>(null);

  // 이벤트는 놓칠 수 있으므로 파일에서 먼저 복원하고, 그 뒤 실시간 이벤트를 잇는다.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [j, text] = await Promise.all([jobsGet(jobId), jobsLog(jobId)]);
        if (cancelled) return;
        setJob(j);
        setLines(
          text
            .split("\n")
            .filter((l) => l.length > 0)
            .map((text) => ({ stream: "stdout" as LogStream, text })),
        );
      } catch (e) {
        if (!cancelled) setError(asAppError(e).message);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [jobId]);

  useEffect(() => {
    const unLog = onLog((e) => {
      if (e.jobId !== jobId) return;
      setLines((prev) => {
        const next = [...prev, { stream: e.stream, text: e.line }];
        return next.length > MAX_LINES ? next.slice(next.length - MAX_LINES) : next;
      });
    });
    const unState = onState((e) => {
      if (e.jobId !== jobId) return;
      void jobsGet(jobId).then(setJob);
      void onChanged();
    });
    const unProgress = onProgress((e) => {
      if (e.jobId === jobId) setProgress({ fraction: e.fraction, label: e.label });
    });
    return () => {
      void unLog.then((f) => f());
      void unState.then((f) => f());
      void unProgress.then((f) => f());
    };
  }, [jobId, onChanged]);

  useEffect(() => {
    if (follow && logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [lines, follow]);

  const cancel = async () => {
    if (!window.confirm("실행 중인 작업을 취소합니다. 진행 중인 계산은 버려집니다. 계속할까요?"))
      return;
    try {
      await jobsCancel(jobId);
    } catch (e) {
      setError(asAppError(e).message);
    }
  };

  const openReport = async () => {
    try {
      await reportOpen(jobId);
    } catch (e) {
      setError(asAppError(e).message);
    }
  };

  const fraction = progress?.fraction ?? job?.progress ?? null;
  const running = job?.status === "running" || job?.status === "queued";
  // 리포트는 회수가 끝나야 존재한다. 실행 중에 눌러도 열 것이 없다.
  const hasReport =
    job?.status === "completed" &&
    job.outputPath != null &&
    (job.module === "SchemaEvaluator" || job.module === "AlleleCallEvaluator");

  return (
    <>
      <div className="page-head">
        <div>
          <button className="link" onClick={onBack}>
            ← 작업 목록
          </button>
          <h1>{job ? MODULE_LABEL[job.module] : "작업"}</h1>
          <p>
            {job && (
              <>
                <span className={`pill ${job.status}`}>{STATUS_LABEL[job.status]}</span>{" "}
                {formatDuration(job.startedAt, job.finishedAt)}
              </>
            )}
          </p>
        </div>
        <div className="row">
          {hasReport && (
            <button className="primary" onClick={() => void openReport()}>
              리포트 열기
            </button>
          )}
          {job?.outputPath && (
            <button onClick={() => void revealItemInDir(job.outputPath!)}>결과 폴더 열기</button>
          )}
          {running && (
            <button className="danger" onClick={() => void cancel()}>
              취소
            </button>
          )}
        </div>
      </div>

      {error && <div className="banner error">{error}</div>}
      {job?.error && <div className="banner error">{job.error}</div>}

      {running && (
        <div className="card tight">
          <div className="row spread">
            <span>{progress?.label ?? "진행 중"}</span>
            <span className="mono">{fraction == null ? "" : `${Math.round(fraction * 100)}%`}</span>
          </div>
          <div className={`progress ${fraction == null ? "indeterminate" : ""}`}>
            <div style={fraction == null ? undefined : { width: `${Math.round(fraction * 100)}%` }} />
          </div>
        </div>
      )}

      <div className="card">
        <div className="row spread" style={{ marginBottom: 8 }}>
          <h2 style={{ margin: 0 }}>로그</h2>
          <label className="inline-check" style={{ margin: 0 }}>
            <input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} />
            자동 스크롤
          </label>
        </div>
        <div className="log" ref={logRef}>
          {lines.length === 0
            ? "아직 출력이 없습니다."
            : lines.map((l, i) => (
                <div key={i} className={l.stream}>
                  {l.text}
                </div>
              ))}
        </div>
      </div>

      {job && (
        <div className="card">
          <h2>상세</h2>
          <table className="kv">
            <tbody>
              <tr>
                <td>작업 ID</td>
                <td className="mono">{job.jobId}</td>
              </tr>
              <tr>
                <td>시작 / 종료</td>
                <td>
                  {formatTime(job.startedAt)} → {formatTime(job.finishedAt)}
                </td>
              </tr>
              <tr>
                <td>종료 코드</td>
                <td>{job.exitCode ?? "—"}</td>
              </tr>
              <tr>
                <td>결과 위치</td>
                <td className="path">{job.outputPath ?? "—"}</td>
              </tr>
              <tr>
                <td>로그 파일</td>
                <td className="path">{job.logPath ?? "—"}</td>
              </tr>
              <tr>
                <td>실행 인자</td>
                <td className="path">{job.args}</td>
              </tr>
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
