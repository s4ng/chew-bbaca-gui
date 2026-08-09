import { useCallback, useEffect, useState } from "react";

import { formatDuration, formatTime, MODULE_LABEL, STATUS_LABEL } from "../lib/format";
import { jobsCancel, jobsList, jobsReconcile, onProgress, onState } from "../lib/ipc";
import { asAppError, type Job } from "../lib/types";
import JobDetail from "./JobDetail";

export default function JobsPage({ onNewJob }: { onNewJob: () => void }) {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** 앱이 꺼진 사이에도 살아남아 있던 작업 (§6.3) */
  const [adopted, setAdopted] = useState<Job[]>([]);
  const [progress, setProgress] = useState<Record<string, { fraction: number; label: string }>>({});

  const refresh = useCallback(async () => {
    try {
      setJobs(await jobsList());
    } catch (e) {
      setError(asAppError(e).message);
    }
  }, []);

  // 조정(reconciliation)은 앱 시작 후 UI 가 준비된 시점에 한 번만 돈다.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const alive = await jobsReconcile();
        if (!cancelled) setAdopted(alive);
      } catch (e) {
        if (!cancelled) setError(asAppError(e).message);
      }
      if (!cancelled) await refresh();
    })();
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  useEffect(() => {
    const unState = onState(() => void refresh());
    const unProgress = onProgress((e) =>
      setProgress((prev) => ({ ...prev, [e.jobId]: { fraction: e.fraction, label: e.label } })),
    );
    return () => {
      void unState.then((f) => f());
      void unProgress.then((f) => f());
    };
  }, [refresh]);

  if (selected) {
    return <JobDetail jobId={selected} onBack={() => setSelected(null)} onChanged={refresh} />;
  }

  return (
    <>
      <div className="page-head">
        <div>
          <h1>작업</h1>
          <p>실행 이력과 진행 상황입니다. 동시에 하나씩 순서대로 실행됩니다.</p>
        </div>
        <button className="primary" onClick={onNewJob}>
          새 작업
        </button>
      </div>

      {error && <div className="banner error">{error}</div>}

      {adopted.map((job) => (
        <div key={job.jobId} className="banner warn">
          <div className="row spread">
            <span>
              이전에 시작한 작업이 아직 실행 중입니다 — {MODULE_LABEL[job.module]} (
              {formatTime(job.startedAt)} 시작)
            </span>
            <span className="row">
              <button onClick={() => setSelected(job.jobId)}>복구</button>
              <button
                className="danger"
                onClick={() => {
                  void jobsCancel(job.jobId).then(() => {
                    setAdopted((prev) => prev.filter((j) => j.jobId !== job.jobId));
                    void refresh();
                  });
                }}
              >
                종료
              </button>
            </span>
          </div>
        </div>
      ))}

      {jobs.length === 0 ? (
        <div className="empty">
          <p>아직 실행한 작업이 없습니다.</p>
          <button className="primary" onClick={onNewJob}>
            첫 작업 만들기
          </button>
        </div>
      ) : (
        <div className="job-list">
          {jobs.map((job) => {
            const live = progress[job.jobId];
            const fraction = live?.fraction ?? job.progress ?? null;
            return (
              <button key={job.jobId} className="job-row" onClick={() => setSelected(job.jobId)}>
                <strong>{MODULE_LABEL[job.module]}</strong>
                <span className={`pill ${job.status}`}>{STATUS_LABEL[job.status]}</span>
                <span className="meta">
                  {formatTime(job.createdAt)} · {formatDuration(job.startedAt, job.finishedAt)}
                  {job.status === "running" && live ? ` · ${live.label}` : ""}
                </span>
                {job.status === "running" && (
                  <div className="meta">
                    <div className={`progress ${fraction == null ? "indeterminate" : ""}`}>
                      <div
                        style={
                          fraction == null ? undefined : { width: `${Math.round(fraction * 100)}%` }
                        }
                      />
                    </div>
                  </div>
                )}
              </button>
            );
          })}
        </div>
      )}
    </>
  );
}
