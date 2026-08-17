import { useEffect, useState } from "react";

import DataDirField from "../components/DataDirField";
import { useT } from "../lib/i18n";
import type { Messages } from "../lib/messages/ko";
import {
  envFirmwareHint,
  envInstallWsl,
  envManualCommands,
  envProvision,
  envRebootToFirmware,
  envRootfsOrigin,
  onProvision,
} from "../lib/ipc";
import {
  asAppError,
  type EnvReport,
  type FirmwareHint,
  type ProvisionEvent,
  type RootfsOrigin,
} from "../lib/types";

/**
 * 온보딩 (ARCHITECTURE.md §7).
 *
 * 화면 하나에 게이트 세 개를 모두 그린다. 사용자는 자기가 어디서 막혔고
 * 무엇이 남았는지를 한눈에 봐야 한다 — 단계마다 화면이 바뀌면 "재부팅 후
 * 뭘 하던 중이었지" 를 매번 다시 파악해야 한다.
 */
export default function Onboarding({
  report,
  checking,
  onRecheck,
}: {
  report: EnvReport | null;
  checking: boolean;
  onRecheck: () => Promise<void> | void;
}) {
  const t = useT();
  const gate = report?.gate ?? "unknown";

  return (
    <div className="onboarding">
      <h1>{t.onboarding.title}</h1>
      <p style={{ color: "var(--text-dim)" }}>
        {t.onboarding.subtitle(report?.distro ?? "chewie-env")}
      </p>

      <GateSteps report={report} />

      {gate === "bios-virtualization" && <BiosGate report={report!} />}
      {gate === "wsl-missing" && <WslGate />}
      {gate === "distro-missing" && <DistroGate onDone={onRecheck} />}
      {gate === "unknown" && <div className="banner warn">{t.onboarding.unknownGate}</div>}

      <div className="row" style={{ marginTop: 18 }}>
        <button onClick={() => void onRecheck()} disabled={checking}>
          {checking ? t.onboarding.checking : t.onboarding.recheck}
        </button>
      </div>

      {report && report.messages.length > 0 && (
        <details className="card" style={{ marginTop: 18 }}>
          <summary>{t.onboarding.diagnostics}</summary>
          <ul className="detail-list">
            {report.messages.map((m, i) => (
              <li key={i}>{m}</li>
            ))}
          </ul>
          <table className="kv" style={{ marginTop: 10 }}>
            <tbody>
              <tr>
                <td>{t.onboarding.hypervisor}</td>
                <td>{String(report.hypervisorPresent ?? t.settings.unknown)}</td>
              </tr>
              <tr>
                <td>{t.onboarding.firmware}</td>
                <td>{String(report.virtualizationFirmwareEnabled ?? t.settings.unknown)}</td>
              </tr>
              <tr>
                <td>{t.onboarding.wslInstalled}</td>
                <td>{report.wslInstalled ? t.onboarding.yes : t.onboarding.no}</td>
              </tr>
              <tr>
                <td>{t.onboarding.existingDistros}</td>
                <td>{report.existingDistros.join(", ") || t.onboarding.noneParen}</td>
              </tr>
              <tr>
                <td>{t.onboarding.vendorModel}</td>
                <td>
                  {report.manufacturer ?? t.common.dash} / {report.model ?? t.common.dash}
                </td>
              </tr>
            </tbody>
          </table>
        </details>
      )}

      <Fallbacks />
    </div>
  );
}

/** 게이트 3개의 통과 여부를 한 줄씩. */
function GateSteps({ report }: { report: EnvReport | null }) {
  const t = useT();
  const gate = report?.gate ?? "unknown";
  const hardwareOk = gate !== "bios-virtualization";
  const wslOk = hardwareOk && gate !== "wsl-missing";
  const distroOk = gate === "ready";

  const steps = [
    {
      state: gate === "bios-virtualization" ? "blocked" : hardwareOk ? "done" : "pending",
      title: t.onboarding.step1,
      desc: t.onboarding.step1Desc,
    },
    {
      state: gate === "wsl-missing" ? "blocked" : wslOk ? "done" : "pending",
      title: t.onboarding.step2,
      desc: t.onboarding.step2Desc,
    },
    {
      state: gate === "distro-missing" ? "blocked" : distroOk ? "done" : "pending",
      title: t.onboarding.step3,
      desc: t.onboarding.step3Desc,
    },
  ];

  return (
    <div className="gate-steps">
      {steps.map((s) => (
        <div key={s.title} className={`gate-step ${s.state}`}>
          <span className="icon">{s.state === "done" ? "✓" : s.state === "blocked" ? "!" : "·"}</span>
          <div>
            <strong>{s.title}</strong>
            <div style={{ color: "var(--text-dim)", fontSize: 13 }}>{s.desc}</div>
          </div>
        </div>
      ))}
    </div>
  );
}

/**
 * BIOS 가상화 안내 (§7.6).
 *
 * 중요한 것은 **이 시점까지 Windows 기능 활성화도 재부팅도 하지 않았다**는 점이다.
 * 사용자는 헛된 재부팅 없이 자기 기기의 상태를 알게 된다.
 */
function BiosGate({ report }: { report: EnvReport }) {
  const t = useT();
  const [hint, setHint] = useState<FirmwareHint | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    envFirmwareHint(report.manufacturer)
      .then(setHint)
      .catch((e) => setError(asAppError(e).message));
  }, [report.manufacturer]);

  const reboot = async () => {
    const ok = window.confirm(t.onboarding.biosRebootConfirm);
    if (!ok) return;
    try {
      await envRebootToFirmware();
    } catch (e) {
      setError(asAppError(e).message);
    }
  };

  // 펌웨어는 켜져 있는데 여기까지 왔다면 BIOS 는 이미 사용자가 해결한 것이다.
  // 같은 화면에 "가상화를 켜세요" 라고 다시 쓰면 사용자는 할 일이 없어진다.
  const firmwareOn = report.virtualizationFirmwareEnabled === true;

  return (
    <div className="card">
      <h2>{firmwareOn ? t.onboarding.biosTitleOn : t.onboarding.biosTitleOff}</h2>
      {firmwareOn ? (
        <>
          <p>{t.onboarding.biosFirmwareOnIntro}</p>
          <ol style={{ color: "var(--text-dim)", lineHeight: 1.7 }}>
            <li>{t.onboarding.biosFirmwareOn1}</li>
            <li>{t.onboarding.biosFirmwareOn2}</li>
            <li>{t.onboarding.biosFirmwareOn3}</li>
          </ol>
          <p style={{ color: "var(--text-dim)" }}>{t.onboarding.biosFirmwareOnNote}</p>
        </>
      ) : (
        <p>{t.onboarding.biosFirmwareOffIntro}</p>
      )}

      {error && <div className="banner error">{error}</div>}

      <h3>{t.onboarding.biosStep1}</h3>
      <p style={{ color: "var(--text-dim)" }}>{t.onboarding.biosStep1Desc}</p>
      <button className="primary" onClick={() => void reboot()}>
        {t.onboarding.biosReboot}
      </button>

      <h3 style={{ marginTop: 18 }}>{t.onboarding.biosStep2}</h3>
      <table className="kv">
        <tbody>
          <tr>
            <td>{t.onboarding.biosVendor}</td>
            <td>{report.manufacturer ?? t.settings.unknown}</td>
          </tr>
          <tr>
            <td>{t.onboarding.biosEntryKey}</td>
            <td>{hint?.entryKey ?? t.common.dash}</td>
          </tr>
          <tr>
            <td>{t.onboarding.biosMenuPath}</td>
            <td>{hint?.menuPath ?? t.common.dash}</td>
          </tr>
        </tbody>
      </table>
      <p style={{ color: "var(--text-dim)", marginTop: 8 }}>{t.onboarding.biosVendorNote}</p>

      <h3>{t.onboarding.biosStep3}</h3>
      <p style={{ color: "var(--text-dim)" }}>{t.onboarding.biosStep3Desc}</p>
    </div>
  );
}

/** WSL 설치 (§7.5). 버튼을 주되 수동 경로를 없애지 않는다. */
function WslGate() {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [denied, setDenied] = useState(false);
  const [commands, setCommands] = useState<string[]>([]);

  useEffect(() => {
    envManualCommands().then(setCommands).catch(() => setCommands([]));
  }, []);

  const install = async () => {
    setBusy(true);
    setMessage(null);
    setDenied(false);
    try {
      setMessage(await envInstallWsl());
    } catch (e) {
      const err = asAppError(e);
      // 권한 상승 거부는 실패가 아니라 **다른 경로로 가라는 신호**다.
      if (err.kind === "elevation-denied") setDenied(true);
      else setMessage(err.message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="card">
      <h2>{t.onboarding.wslTitle}</h2>
      <p>{t.onboarding.wslIntro}</p>

      <button className="primary" onClick={() => void install()} disabled={busy}>
        {busy ? t.onboarding.wslInstalling : t.onboarding.wslInstall}
      </button>

      {message && <div className="banner info" style={{ marginTop: 12 }}>{message}</div>}

      {denied && (
        <div style={{ marginTop: 14 }}>
          <div className="banner warn">{t.onboarding.wslDenied}</div>
          <p style={{ color: "var(--text-dim)" }}>{t.onboarding.wslDeniedHow}</p>
          {commands.map((c) => (
            <CopyBox key={c} text={c} />
          ))}
        </div>
      )}

      <p style={{ color: "var(--text-dim)", marginTop: 12 }}>{t.onboarding.wslAfter}</p>
    </div>
  );
}

/** rootfs 확보 → 검증 → import (§7.3 ③). */
function DistroGate({ onDone }: { onDone: () => Promise<void> | void }) {
  const t = useT();
  const [running, setRunning] = useState(false);
  const [event, setEvent] = useState<ProvisionEvent | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [origin, setOrigin] = useState<RootfsOrigin | null>(null);
  /**
   * 등록까지 끝났다. `running` 을 끄는 것만으로는 부족하다 — 부모가 환경을 다시
   * 검사해 화면을 바꾸기까지 짧은 틈이 있고, 그 사이에 [설치] 버튼이 되살아나
   * 이미 끝난 일을 다시 시킬 것처럼 보인다.
   */
  const [done, setDone] = useState(false);

  useEffect(() => {
    envRootfsOrigin()
      .then(setOrigin)
      .catch(() => setOrigin(null));
  }, []);

  useEffect(() => {
    const un = onProvision((e) => {
      setEvent(e);
      if (e.ok === false) {
        setError(e.message);
        setRunning(false);
      }
      if (e.ok === true) {
        setRunning(false);
        setDone(true);
        void onDone();
      }
    });
    return () => {
      void un.then((f) => f());
    };
  }, [onDone]);

  const start = async () => {
    setError(null);
    setRunning(true);
    try {
      await envProvision();
    } catch (e) {
      setError(asAppError(e).message);
      setRunning(false);
    }
  };

  const fraction = event?.fraction ?? null;
  const remote = origin === "remote";
  const missing = origin === "missing";

  return (
    <div className="card">
      <h2>{remote ? t.onboarding.distroTitleRemote : t.onboarding.distroTitleLocal}</h2>
      <p>{t.onboarding.distroIntro(remote, !remote && !missing)}</p>

      {missing && <div className="banner warn">{t.onboarding.distroMissing}</div>}

      {error && <div className="banner error">{error}</div>}

      {/*
        설치 위치는 **여기서 고르는 것이 마지막 기회**다. `wsl --import` 로 등록하고
        나면 가상 디스크가 그 경로에 묶여, 옮기려면 배포판을 지우고 다시 깔아야 한다.
        그래서 [설치] 버튼 바로 위에 둔다.
      */}
      {!running && !done && <DataDirField />}

      {running || event ? (
        <div style={{ marginTop: 12 }}>
          <div className="row spread">
            <span>{stageLabel(t, event?.stage)}</span>
            <span className="mono">{event?.message}</span>
          </div>
          <div className={`progress ${fraction == null ? "indeterminate" : ""}`}>
            <div style={fraction == null ? undefined : { width: `${Math.round(fraction * 100)}%` }} />
          </div>
        </div>
      ) : null}

      {done ? (
        <p style={{ color: "var(--text-dim)", marginTop: 12 }}>{t.onboarding.distroDone}</p>
      ) : (
        !running &&
        !missing && (
          <button className="primary" onClick={() => void start()} style={{ marginTop: 12 }}>
            {remote ? t.onboarding.distroInstallRemote : t.onboarding.distroInstall}
          </button>
        )
      )}
    </div>
  );
}

function stageLabel(t: Messages, stage?: ProvisionEvent["stage"]): string {
  switch (stage) {
    case "download":
      return t.onboarding.stageDownload;
    case "verify":
      return t.onboarding.stageVerify;
    case "import":
      return t.onboarding.stageImport;
    case "done":
      return t.onboarding.stageDone;
    default:
      return t.onboarding.stageIdle;
  }
}

/** 끝내 불가능한 사용자를 위한 대안 (§7.7). "실행할 수 없습니다" 로 끝내지 않는다. */
function Fallbacks() {
  const t = useT();
  return (
    <details className="card">
      <summary>{t.onboarding.fallbackTitle}</summary>
      <p style={{ marginTop: 10 }}>{t.onboarding.fallbackIntro}</p>
      <ul>
        <li>{t.onboarding.fallbackGalaxy}</li>
        <li>{t.onboarding.fallbackViewer}</li>
      </ul>
    </details>
  );
}

function CopyBox({ text }: { text: string }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  return (
    <div className="copy-box">
      <code>{text}</code>
      <button
        onClick={() => {
          void navigator.clipboard.writeText(text).then(() => {
            setCopied(true);
            setTimeout(() => setCopied(false), 1500);
          });
        }}
      >
        {copied ? t.onboarding.copied : t.onboarding.copy}
      </button>
    </div>
  );
}
