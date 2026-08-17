import { useCallback, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";

import { envProbe, guideOpen } from "./lib/ipc";
import { asAppError, type EnvReport } from "./lib/types";
import JobsPage from "./routes/JobsPage";
import NewJobPage from "./routes/NewJobPage";
import Onboarding from "./routes/Onboarding";
import SchemasPage from "./routes/SchemasPage";
import SettingsPage from "./routes/SettingsPage";

type View = "jobs" | "new" | "schemas" | "settings";

/**
 * chewBBACA 공식 문서. 앱이 답하지 않는 질문(모듈 인자의 의미, 분류 코드 해석 등)은
 * 결국 여기로 가야 한다.
 *
 * `<a href>` 를 쓰면 웹뷰가 앱 밖으로 이동해 돌아올 수 없다. 반드시 기본 브라우저로
 * 연다 — `opener:default` 에 https 스코프가 포함돼 있어 추가 권한 설정은 필요 없다.
 */
const DOCS_URL = "https://chewbbaca.readthedocs.io/en/latest/";

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
  /** 문서 열기가 실패하면 조용히 넘어가지 않는다 — 아무 반응이 없으면 원인을 알 수 없다. */
  const [linkError, setLinkError] = useState<string | null>(null);
  /**
   * 앱 버전. `tauri.conf.json` 의 값을 그대로 읽으므로 인스톨러 파일명과 언제나
   * 같다 — 사용자가 "어느 버전을 쓰고 있는지" 를 물어올 때 이것이 답이어야 한다.
   */
  const [version, setVersion] = useState<string | null>(null);

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

  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

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
          {/* 가이드는 앱에 묻어 나가므로 인터넷 없이도 열린다. */}
          <button
            className="doc-link"
            onClick={() => {
              setLinkError(null);
              void guideOpen().catch((e) => setLinkError(asAppError(e).message));
            }}
            title="예제 데이터로 전 과정을 따라가 보는 안내서"
          >
            따라해보기 ↗
          </button>
          <button
            className="doc-link"
            onClick={() => {
              setLinkError(null);
              void openUrl(DOCS_URL).catch((e) => setLinkError(asAppError(e).message));
            }}
            title={DOCS_URL}
          >
            chewBBACA 공식 문서 ↗
          </button>
          {linkError && <div className="link-error">{linkError}</div>}
          <div className="distro">
            배포판 <code>{report.distro}</code>
          </div>
          {/* 버그 신고를 받을 때 가장 먼저 물어보는 값이라 항상 보이는 자리에 둔다. */}
          {version && <div className="app-version">버전 {version}</div>}
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
