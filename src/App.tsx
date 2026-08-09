import { useCallback, useEffect, useState } from "react";

import { envProbe } from "./lib/ipc";
import { asAppError, type EnvReport } from "./lib/types";
import JobsPage from "./routes/JobsPage";
import NewJobPage from "./routes/NewJobPage";
import Onboarding from "./routes/Onboarding";
import SchemasPage from "./routes/SchemasPage";
import SettingsPage from "./routes/SettingsPage";

type View = "jobs" | "new" | "schemas" | "settings";

const NAV: { id: View; label: string }[] = [
  { id: "jobs", label: "작업" },
  { id: "new", label: "새 작업" },
  { id: "schemas", label: "스키마" },
  { id: "settings", label: "설정" },
];

export default function App() {
  const [report, setReport] = useState<EnvReport | null>(null);
  const [probeError, setProbeError] = useState<string | null>(null);
  const [checking, setChecking] = useState(true);
  const [view, setView] = useState<View>("jobs");

  const recheck = useCallback(async () => {
    setChecking(true);
    setProbeError(null);
    try {
      setReport(await envProbe());
    } catch (e) {
      setProbeError(asAppError(e).message);
    } finally {
      setChecking(false);
    }
  }, []);

  useEffect(() => {
    void recheck();
  }, [recheck]);

  // 첫 판정 전에는 아무것도 묻지 않는다. 정상 환경이면 이 화면은 한순간만 보인다.
  if (checking && !report && !probeError) {
    return (
      <div className="onboarding">
        <h1>환경을 확인하는 중...</h1>
        <div className="progress indeterminate">
          <div />
        </div>
      </div>
    );
  }

  if (probeError) {
    return (
      <div className="onboarding">
        <div className="banner error">환경 검사에 실패했습니다: {probeError}</div>
        <button className="primary" onClick={() => void recheck()}>
          다시 검사
        </button>
      </div>
    );
  }

  if (!report || report.gate !== "ready") {
    return <Onboarding report={report} checking={checking} onRecheck={recheck} />;
  }

  return (
    <div className="app">
      <nav className="sidebar">
        <div className="brand">
          <span>chewBBACA</span>
          <small>Desktop</small>
        </div>
        {NAV.map((item) => (
          <button
            key={item.id}
            className="nav-item"
            aria-current={view === item.id}
            onClick={() => setView(item.id)}
          >
            {item.label}
          </button>
        ))}
        <div className="sidebar-footer">
          배포판 <code>{report.distro}</code>
        </div>
      </nav>

      <main className="content">
        {view === "jobs" && <JobsPage onNewJob={() => setView("new")} />}
        {view === "new" && <NewJobPage onSubmitted={() => setView("jobs")} />}
        {view === "schemas" && <SchemasPage />}
        {view === "settings" && <SettingsPage onEnvChanged={recheck} />}
      </main>
    </div>
  );
}
