import { useEffect, useState } from "react";

import DataDirField from "../components/DataDirField";
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
  const gate = report?.gate ?? "unknown";

  return (
    <div className="onboarding">
      <h1>실행 환경 준비</h1>
      <p style={{ color: "var(--text-dim)" }}>
        chewBBACA 는 Linux 에서만 동작합니다. 이 앱은 전용 WSL2 배포판(
        <code>{report?.distro ?? "chewie-env"}</code>)을 하나 만들어 그 안에서 실행합니다.
        기존 WSL 배포판과 전역 설정은 건드리지 않습니다.
      </p>

      <GateSteps report={report} />

      {gate === "bios-virtualization" && <BiosGate report={report!} />}
      {gate === "wsl-missing" && <WslGate />}
      {gate === "distro-missing" && <DistroGate onDone={onRecheck} />}
      {gate === "unknown" && (
        <div className="banner warn">
          환경을 판정하지 못했습니다. 아래 진단 정보를 확인하거나 프로젝트의
          <code> scripts/check-env.bat </code>
          를 실행해 주세요.
        </div>
      )}

      <div className="row" style={{ marginTop: 18 }}>
        <button onClick={() => void onRecheck()} disabled={checking}>
          {checking ? "검사 중..." : "다시 검사"}
        </button>
      </div>

      {report && report.messages.length > 0 && (
        <details className="card" style={{ marginTop: 18 }}>
          <summary>진단 정보</summary>
          <ul className="detail-list">
            {report.messages.map((m, i) => (
              <li key={i}>{m}</li>
            ))}
          </ul>
          <table className="kv" style={{ marginTop: 10 }}>
            <tbody>
              <tr>
                <td>HypervisorPresent</td>
                <td>{String(report.hypervisorPresent ?? "확인 불가")}</td>
              </tr>
              <tr>
                <td>펌웨어 가상화</td>
                <td>{String(report.virtualizationFirmwareEnabled ?? "확인 불가")}</td>
              </tr>
              <tr>
                <td>WSL 설치</td>
                <td>{report.wslInstalled ? "예" : "아니오"}</td>
              </tr>
              <tr>
                <td>기존 배포판</td>
                <td>{report.existingDistros.join(", ") || "(없음)"}</td>
              </tr>
              <tr>
                <td>제조사 / 모델</td>
                <td>
                  {report.manufacturer ?? "—"} / {report.model ?? "—"}
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
  const gate = report?.gate ?? "unknown";
  const hardwareOk = gate !== "bios-virtualization";
  const wslOk = hardwareOk && gate !== "wsl-missing";
  const distroOk = gate === "ready";

  const steps = [
    {
      state: gate === "bios-virtualization" ? "blocked" : hardwareOk ? "done" : "pending",
      title: "① 하드웨어 가상화",
      desc: "CPU 가상화가 켜져 있고 하이퍼바이저가 동작하는지 확인합니다.",
    },
    {
      state: gate === "wsl-missing" ? "blocked" : wslOk ? "done" : "pending",
      title: "② WSL",
      desc: "WSL2 가 설치되어 있어야 합니다. 설치에는 관리자 권한과 재부팅이 필요합니다.",
    },
    {
      state: gate === "distro-missing" ? "blocked" : distroOk ? "done" : "pending",
      title: "③ 전용 배포판",
      desc: "앱에 포함된 chewBBACA 이미지를 전용 배포판으로 등록합니다.",
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
  const [hint, setHint] = useState<FirmwareHint | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    envFirmwareHint(report.manufacturer)
      .then(setHint)
      .catch((e) => setError(asAppError(e).message));
  }, [report.manufacturer]);

  const reboot = async () => {
    const ok = window.confirm(
      "지금 재부팅하고 UEFI 설정 화면으로 들어갑니다.\n저장하지 않은 작업이 있으면 먼저 저장하세요.\n계속할까요?",
    );
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
      <h2>{firmwareOn ? "가상화가 동작하지 않습니다" : "CPU 가상화가 꺼져 있습니다"}</h2>
      {firmwareOn ? (
        <>
          <p>
            펌웨어(BIOS/UEFI)의 가상화는 <strong>켜져 있는 것으로 확인</strong>되는데
            하이퍼바이저가 동작하지 않습니다. 남은 원인은 Windows 쪽입니다.
          </p>
          <ol style={{ color: "var(--text-dim)", lineHeight: 1.7 }}>
            <li>
              관리자 PowerShell 에서 <code>wsl --install --no-distribution</code> 을 실행하고
              재부팅합니다. (Virtual Machine Platform 기능이 켜집니다.)
            </li>
            <li>
              그래도 같다면 관리자 PowerShell 에서{" "}
              <code>bcdedit /set hypervisorlaunchtype auto</code> 실행 후 재부팅합니다.
              하이퍼바이저 기동이 꺼져 있는 경우입니다.
            </li>
            <li>
              사내 보안 정책이나 다른 가상화 소프트웨어(VMware/VirtualBox 구버전)가 막고 있을
              수도 있습니다.
            </li>
          </ol>
          <p style={{ color: "var(--text-dim)" }}>
            아래 펌웨어 안내는 위 방법이 모두 실패했을 때 확인용으로 남겨 둡니다.
          </p>
        </>
      ) : (
        <p>
          Windows 11 이라고 해서 가상화가 켜져 있는 것은 아닙니다. 최소 요구사항(TPM 2.0,
          Secure Boot)에 가상화는 포함되지 않습니다. 펌웨어(BIOS/UEFI)에서 켜야 합니다.
        </p>
      )}

      {error && <div className="banner error">{error}</div>}

      <h3>1. 펌웨어로 바로 들어가기</h3>
      <p style={{ color: "var(--text-dim)" }}>
        재부팅과 동시에 UEFI 설정으로 진입합니다. 부팅 중 키를 연타할 필요가 없습니다.
        (레거시 BIOS 기기에서는 동작하지 않습니다 — 아래 수동 방법을 쓰세요.)
      </p>
      <button className="primary" onClick={() => void reboot()}>
        재부팅하고 UEFI 열기
      </button>

      <h3 style={{ marginTop: 18 }}>2. 직접 들어가기</h3>
      <table className="kv">
        <tbody>
          <tr>
            <td>제조사</td>
            <td>{report.manufacturer ?? "확인 불가"}</td>
          </tr>
          <tr>
            <td>진입 키</td>
            <td>{hint?.entryKey ?? "—"}</td>
          </tr>
          <tr>
            <td>설정 위치</td>
            <td>{hint?.menuPath ?? "—"}</td>
          </tr>
        </tbody>
      </table>
      <p style={{ color: "var(--text-dim)", marginTop: 8 }}>
        설정 항목 이름은 제조사마다 다릅니다. Intel 은 <code>Intel Virtualization Technology</code>
        /<code>VT-x</code>, AMD 는 <code>SVM Mode</code> 입니다.
      </p>

      <h3>3. 확인 방법</h3>
      <p style={{ color: "var(--text-dim)" }}>
        작업 관리자 → 성능 → CPU 에서 <strong>가상화: 사용</strong> 이면 켜진 것입니다.
        켠 뒤 이 화면에서 [다시 검사] 를 누르세요.
      </p>
    </div>
  );
}

/** WSL 설치 (§7.5). 버튼을 주되 수동 경로를 없애지 않는다. */
function WslGate() {
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
      <h2>WSL 설치가 필요합니다</h2>
      <p>
        관리자 권한과 재부팅이 필요합니다. 아래 버튼을 누르면 권한 상승 창(UAC)이 뜨고,
        앱 본체는 계속 일반 권한으로 남습니다. 다른 Linux 배포판은 설치하지 않습니다.
      </p>

      <button className="primary" onClick={() => void install()} disabled={busy}>
        {busy ? "설치 중..." : "WSL 설치"}
      </button>

      {message && <div className="banner info" style={{ marginTop: 12 }}>{message}</div>}

      {denied && (
        <div style={{ marginTop: 14 }}>
          <div className="banner warn">
            권한 상승이 거부되었습니다. 아래 명령을 <strong>관리자 PowerShell</strong> 에서 직접
            실행해도 됩니다.
          </div>
          <p style={{ color: "var(--text-dim)" }}>
            시작 → &quot;PowerShell&quot; 우클릭 → <strong>관리자 권한으로 실행</strong>
          </p>
          {commands.map((c) => (
            <CopyBox key={c} text={c} />
          ))}
        </div>
      )}

      <p style={{ color: "var(--text-dim)", marginTop: 12 }}>
        설치가 끝나면 <strong>재부팅</strong>한 뒤 이 앱을 다시 실행하세요. 중단된 지점을
        기억할 필요는 없습니다 — 다시 켜면 이어서 진행됩니다.
      </p>
    </div>
  );
}

/** rootfs 확보 → 검증 → import (§7.3 ③). */
function DistroGate({ onDone }: { onDone: () => Promise<void> | void }) {
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
      <h2>{remote ? "chewBBACA 환경 내려받기" : "chewBBACA 환경 설치"}</h2>
      <p>
        chewBBACA 와 BLAST+ / MAFFT / FastTree 가 들어 있는 이미지를
        {remote ? " 내려받아 " : " "}
        전용 배포판으로 등록합니다. 한 번만 하면 됩니다.
        {!remote && !missing && " 이미지는 앱에 포함되어 있어 인터넷 연결이 필요 없습니다."}
      </p>

      {missing && (
        <div className="banner warn">
          앱에 포함된 rootfs 이미지를 찾을 수 없습니다. 인스톨러로 설치한 앱이라면 다시 설치해
          주세요. 개발 중이라면 [설정] → rootfs 이미지 칸에 직접 빌드한 tar.gz 경로를 넣으면
          됩니다.
        </div>
      )}

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
            <span>{stageLabel(event?.stage)}</span>
            <span className="mono">{event?.message}</span>
          </div>
          <div className={`progress ${fraction == null ? "indeterminate" : ""}`}>
            <div style={fraction == null ? undefined : { width: `${Math.round(fraction * 100)}%` }} />
          </div>
        </div>
      ) : null}

      {done ? (
        <p style={{ color: "var(--text-dim)", marginTop: 12 }}>
          환경이 준비되었습니다. 잠시 후 앱으로 들어갑니다...
        </p>
      ) : (
        !running &&
        !missing && (
          <button className="primary" onClick={() => void start()} style={{ marginTop: 12 }}>
            {remote ? "내려받고 설치" : "설치"}
          </button>
        )
      )}
    </div>
  );
}

function stageLabel(stage?: ProvisionEvent["stage"]): string {
  switch (stage) {
    case "download":
      return "내려받는 중";
    case "verify":
      return "체크섬 검증";
    case "import":
      return "배포판 등록 중";
    case "done":
      return "완료";
    default:
      return "준비 중";
  }
}

/** 끝내 불가능한 사용자를 위한 대안 (§7.7). "실행할 수 없습니다" 로 끝내지 않는다. */
function Fallbacks() {
  return (
    <details className="card">
      <summary>환경을 구성할 수 없는 경우</summary>
      <p style={{ marginTop: 10 }}>
        BIOS 에 관리자 암호가 걸린 회사 장비처럼 끝내 불가능한 경우가 있습니다. 그때는 다음
        두 가지를 쓸 수 있습니다.
      </p>
      <ul>
        <li>
          <strong>Galaxy 웹 버전</strong> — usegalaxy.eu 에 chewBBACA 모듈(CreateSchema,
          AlleleCall, DownloadSchema, PrepExternalSchema)이 등록되어 브라우저에서 실행할 수
          있습니다. 다만 버전이 최신보다 뒤처질 수 있습니다.
        </li>
        <li>
          <strong>결과 뷰어 모드</strong> — 다른 PC 에서 생성된 HTML 리포트와 TSV 를 이 앱으로
          열람할 수 있습니다. <em>(v0.2 예정)</em>
        </li>
      </ul>
    </details>
  );
}

function CopyBox({ text }: { text: string }) {
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
        {copied ? "복사됨" : "복사"}
      </button>
    </div>
  );
}
