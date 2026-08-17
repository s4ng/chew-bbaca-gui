import { useEffect, useRef, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

import { formatDuration, formatTime } from "../lib/format";
import { useT } from "../lib/i18n";
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
  const t = useT();
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
    if (!window.confirm(t.jobDetail.confirmCancel)) return;
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
            {t.jobDetail.back}
          </button>
          <h1>{job ? t.module[job.module] : t.jobDetail.fallbackTitle}</h1>
          <p>
            {job && (
              <>
                <span className={`pill ${job.status}`}>{t.status[job.status]}</span>{" "}
                {formatDuration(job.startedAt, job.finishedAt, t)}
              </>
            )}
          </p>
        </div>
        <div className="row">
          {hasReport && (
            <button className="primary" onClick={() => void openReport()}>
              {t.jobDetail.openReport}
            </button>
          )}
          {job?.outputPath && (
            <button onClick={() => void revealItemInDir(job.outputPath!)}>
              {t.jobDetail.openOutput}
            </button>
          )}
          {running && (
            <button className="danger" onClick={() => void cancel()}>
              {t.jobDetail.cancel}
            </button>
          )}
        </div>
      </div>

      {error && <div className="banner error">{error}</div>}
      {job?.error && <div className="banner error">{job.error}</div>}

      {running && (
        <div className="card tight">
          <div className="row spread">
            <span>{progress?.label ?? t.jobDetail.running}</span>
            <span className="mono">{fraction == null ? "" : `${Math.round(fraction * 100)}%`}</span>
          </div>
          <div className={`progress ${fraction == null ? "indeterminate" : ""}`}>
            <div style={fraction == null ? undefined : { width: `${Math.round(fraction * 100)}%` }} />
          </div>
        </div>
      )}

      <div className="card">
        <div className="row spread" style={{ marginBottom: 8 }}>
          <h2 style={{ margin: 0 }}>{t.jobDetail.log}</h2>
          <label className="inline-check" style={{ margin: 0 }}>
            <input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} />
            {t.jobDetail.autoScroll}
          </label>
        </div>
        <div className="log" ref={logRef}>
          {lines.length === 0
            ? t.jobDetail.noOutput
            : lines.map((l, i) => (
                <div key={i} className={l.stream}>
                  {l.text}
                </div>
              ))}
        </div>
      </div>

      {job && (
        <div className="card">
          <h2>{t.jobDetail.details}</h2>
          <table className="kv">
            <tbody>
              <tr>
                <td>{t.jobDetail.jobId}</td>
                <td className="mono">{job.jobId}</td>
              </tr>
              <tr>
                <td>{t.jobDetail.startedFinished}</td>
                <td>
                  {formatTime(job.startedAt, t)} → {formatTime(job.finishedAt, t)}
                </td>
              </tr>
              <tr>
                <td>{t.jobDetail.exitCode}</td>
                <td>{job.exitCode ?? t.common.dash}</td>
              </tr>
              <tr>
                <td>{t.jobDetail.outputPath}</td>
                <td className="path">{job.outputPath ?? t.common.dash}</td>
              </tr>
              <tr>
                <td>{t.jobDetail.logPath}</td>
                <td className="path">{job.logPath ?? t.common.dash}</td>
              </tr>
              <tr>
                <td>{t.jobDetail.args}</td>
                <td className="path">{job.args}</td>
              </tr>
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
