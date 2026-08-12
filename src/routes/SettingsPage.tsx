import { useEffect, useState } from "react";

import { formatBytes } from "../lib/format";
import {
  backendStatus,
  diskCompact,
  diskUsage,
  envUnregister,
  mcpConfigure,
  mcpGuideOpen,
  mcpRegenerateToken,
  mcpStatus,
  settingsGet,
  settingsSet,
} from "../lib/ipc";
import {
  asAppError,
  type BackendStatus,
  type DiskUsage,
  type McpStatus,
  type Settings,
} from "../lib/types";

export default function SettingsPage({ onEnvChanged }: { onEnvChanged: () => Promise<void> | void }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [backend, setBackend] = useState<BackendStatus | null>(null);
  const [disk, setDisk] = useState<DiskUsage | null>(null);
  const [mcp, setMcp] = useState<McpStatus | null>(null);
  /** 입력 중인 포트. 서버는 포커스를 잃을 때만 다시 띄운다. */
  const [port, setPort] = useState(8787);
  /** 실제로 적용된 포트. 이것과 다를 때만 재기동한다. */
  const [configuredPort, setConfiguredPort] = useState(8787);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void settingsGet()
      .then((s) => {
        setSettings(s);
        setPort(s.mcp.port);
        setConfiguredPort(s.mcp.port);
      })
      .catch((e) => setError(asAppError(e).message));
    void backendStatus().then(setBackend).catch(() => setBackend(null));
    void diskUsage().then(setDisk).catch(() => setDisk(null));
    void mcpStatus().then(setMcp).catch(() => setMcp(null));
  }, []);

  /// MCP 설정은 Rust 쪽에서 DB 에 직접 쓴다. **여기서 `settings` 를 다시 읽지 않으면**
  /// 화면이 들고 있던 낡은 값이 남고, 사용자가 다른 항목을 저장하는 순간 그 낡은
  /// mcp 값(포트·토큰)으로 덮여 쓴다 — 다음 실행에서 조용히 옛 포트로 돌아간다.
  const refreshSettings = async () => {
    const s = await settingsGet().catch(() => null);
    if (s) setSettings(s);
  };

  const applyMcp = async (enabled: boolean, nextPort: number, allowRun: boolean) => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      setMcp(await mcpConfigure(enabled, nextPort, allowRun));
      setConfiguredPort(nextPort);
    } catch (e) {
      // 포트 충돌 등으로 못 띄운 경우에도 설정값은 저장되어 있다.
      setError(asAppError(e).message);
      setMcp(await mcpStatus().catch(() => null));
    } finally {
      await refreshSettings();
      setBusy(false);
    }
  };

  const regenerate = async () => {
    const ok = window.confirm(
      "새 토큰을 발급합니다.\n지금까지 배포한 클라이언트 설정은 즉시 접속할 수 없게 되며, 새 설정을 다시 붙여넣어야 합니다.\n계속할까요?",
    );
    if (!ok) return;
    setBusy(true);
    try {
      setMcp(await mcpRegenerateToken());
      setMessage("새 토큰을 발급했습니다. 아래 설정을 클라이언트에 다시 붙여넣으세요.");
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      // 새 토큰이 화면의 settings 에도 반영되어야 한다 (applyMcp 의 주석 참조).
      await refreshSettings();
      setBusy(false);
    }
  };

  const openMcpGuide = async () => {
    try {
      await mcpGuideOpen();
    } catch (e) {
      setError(asAppError(e).message);
    }
  };

  /**
   * 칸 하나를 클립보드로. 등록 화면이 URL·헤더 키·헤더 값을 따로 받으므로
   * 복사도 칸 단위여야 한다 — 설정 파일 문법을 통째로 주면 사용자가 값을 눈으로
   * 뜯어내야 한다.
   */
  const copyValue = async (label: string, value: string, elementId: string) => {
    setError(null);
    try {
      await navigator.clipboard.writeText(value);
      setMessage(`${label} 복사했습니다.`);
    } catch {
      // 웹뷰가 보안 컨텍스트가 아니면 클립보드 API 가 없다. 그때는 직접 고르게 한다.
      const el = document.getElementById(elementId) as
        | HTMLInputElement
        | HTMLTextAreaElement
        | null;
      el?.select();
      setMessage("복사하지 못했습니다. 칸이 선택되었으니 Ctrl+C 를 누르세요.");
    }
  };

  const save = async (next: Settings) => {
    setSettings(next);
    try {
      await settingsSet(next);
      setMessage("저장했습니다.");
    } catch (e) {
      setError(asAppError(e).message);
    }
  };

  const compact = async () => {
    setBusy(true);
    setMessage(null);
    setError(null);
    try {
      setMessage(await diskCompact());
      setDisk(await diskUsage());
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const removeEnv = async () => {
    const ok = window.confirm(
      "전용 배포판을 제거합니다.\n앱이 소유한 스키마도 함께 삭제됩니다. 필요하면 먼저 [스키마] 화면에서 내보내세요.\n되돌릴 수 없습니다. 계속할까요?",
    );
    if (!ok) return;
    setBusy(true);
    try {
      await envUnregister();
      setMessage("배포판을 제거했습니다.");
      await onEnvChanged();
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(false);
    }
  };

  if (!settings) {
    return <div className="empty">설정을 불러오는 중...</div>;
  }

  return (
    <>
      <div className="page-head">
        <div>
          <h1>설정</h1>
          <p>앱이 소유한 것만 다룹니다. 전역 WSL 설정(.wslconfig)은 수정하지 않습니다.</p>
        </div>
      </div>

      {error && <div className="banner error">{error}</div>}
      {message && <div className="banner info">{message}</div>}

      <div className="card">
        <h2>실행 환경</h2>
        <table className="kv">
          <tbody>
            <tr>
              <td>배포판</td>
              <td className="mono">{settings.distro}</td>
            </tr>
            <tr>
              <td>chewBBACA</td>
              <td>{backend?.chewbbacaVersion ?? "확인 불가"}</td>
            </tr>
            <tr>
              <td>CPU 코어</td>
              <td>{backend?.cpuCount ?? "—"}</td>
            </tr>
            <tr>
              <td>상태</td>
              <td>{backend?.detail ?? "—"}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div className="card">
        <h2>실행</h2>
        <div className="field">
          <label htmlFor="cpu">기본 CPU 개수</label>
          <input
            id="cpu"
            type="number"
            min={1}
            value={settings.defaultCpu ?? ""}
            placeholder="비우면 자동 (WSL nproc)"
            onChange={(e) =>
              void save({
                ...settings,
                defaultCpu: e.target.value ? Number(e.target.value) : null,
              })
            }
          />
        </div>
        <label className="inline-check">
          <input
            type="checkbox"
            checked={settings.keepWorkDir}
            onChange={(e) => void save({ ...settings, keepWorkDir: e.target.checked })}
          />
          완료 후 임시 작업 폴더를 남겨둔다 (디버깅용)
        </label>
      </div>

      <div className="card">
        <h2>디스크</h2>
        <p style={{ color: "var(--text-dim)" }}>
          가상 디스크는 파일을 지워도 자동으로 줄지 않습니다. 대용량 분석 뒤 Windows 여유
          공간이 돌아오지 않으면 아래 버튼으로 정리하세요. 정리 중에는 배포판이 종료됩니다.
        </p>
        <table className="kv">
          <tbody>
            <tr>
              <td>가상 디스크</td>
              <td>{formatBytes(disk?.vhdxBytes ?? null)}</td>
            </tr>
            <tr>
              <td>앱 폴더</td>
              <td className="path">{disk?.appDir ?? "—"}</td>
            </tr>
          </tbody>
        </table>
        <button onClick={() => void compact()} disabled={busy} style={{ marginTop: 10 }}>
          디스크 정리
        </button>
      </div>

      <div className="card">
        <h2>rootfs 이미지</h2>
        <p style={{ color: "var(--text-dim)" }}>
          chewBBACA 이미지는 <strong>앱에 포함되어 배포</strong>됩니다. 아래 칸은 비워 두는 것이
          정상이고, 직접 빌드한 rootfs 로 바꿔 쓸 때만 채우면 됩니다.
        </p>
        <div className="field">
          <label htmlFor="url">파일 경로 또는 URL (비우면 포함된 이미지 사용)</label>
          <input
            id="url"
            type="text"
            value={settings.rootfs.url}
            onChange={(e) =>
              setSettings({ ...settings, rootfs: { ...settings.rootfs, url: e.target.value } })
            }
            onBlur={() => void save(settings)}
          />
          <div className="hint">
            로컬 tar.gz 경로를 넣으면 그 파일을 그대로 검증해 등록하고, http(s) 주소를 넣으면
            내려받습니다 (예: C:\…\dist-rootfs\chewie-rootfs-3.5.4.tar.gz). 값을 넣으면
            앱에 포함된 이미지 대신 이쪽을 씁니다 — 체크섬도 함께 바꿔야 합니다.
          </div>
        </div>
        <div className="field">
          <label htmlFor="sha">SHA256</label>
          <input
            id="sha"
            type="text"
            value={settings.rootfs.sha256}
            onChange={(e) =>
              setSettings({ ...settings, rootfs: { ...settings.rootfs, sha256: e.target.value } })
            }
            onBlur={() => void save(settings)}
          />
          <div className="hint">64자리 16진수. 일치하지 않으면 받은 파일을 폐기합니다.</div>
        </div>
      </div>

      <div className="card">
        <h2>MCP 서버</h2>
        <p style={{ color: "var(--text-dim)" }}>
          ChatGPT 데스크톱 앱 같은 MCP 클라이언트가 이 앱의 기능을 읽고 실행할 수 있게 합니다.
          서버는 <strong>이 앱이 켜져 있는 동안에만</strong> 동작하고, 같은 PC(127.0.0.1)에서만
          접속할 수 있습니다.
        </p>
        <table className="kv">
          <tbody>
            <tr>
              <td>상태</td>
              <td>
                {!mcp
                  ? "확인 중..."
                  : mcp.running
                    ? `실행 중 · ${mcp.url}`
                    : mcp.enabled
                      ? "시작하지 못했습니다 (포트 충돌일 수 있습니다)"
                      : "꺼져 있음"}
              </td>
            </tr>
          </tbody>
        </table>

        <label className="inline-check" style={{ marginTop: 10 }}>
          <input
            type="checkbox"
            checked={mcp?.enabled ?? false}
            disabled={!mcp || busy}
            onChange={(e) => void applyMcp(e.target.checked, port, mcp?.allowRun ?? true)}
          />
          MCP 서버 사용
        </label>
        <label className="inline-check">
          <input
            type="checkbox"
            checked={mcp?.allowRun ?? false}
            disabled={!mcp || busy}
            onChange={(e) => void applyMcp(mcp?.enabled ?? true, port, e.target.checked)}
          />
          작업 실행 허용 (끄면 읽기 전용이 됩니다)
        </label>
        <div className="hint" style={{ marginBottom: 10 }}>
          켜 두면 클라이언트가 요청한 작업이 <strong>앱에서 다시 묻지 않고</strong> 큐에 들어갑니다.
          클라이언트 쪽에도 별도의 도구 승인 설정이 있을 수 있습니다.
        </div>

        <div className="field">
          <label htmlFor="mcp-port">포트</label>
          <input
            id="mcp-port"
            type="number"
            min={1024}
            max={65535}
            value={port}
            disabled={busy}
            onChange={(e) => setPort(Number(e.target.value))}
            onBlur={() => {
              if (mcp && port !== configuredPort && port >= 1024 && port <= 65535) {
                void applyMcp(mcp.enabled, port, mcp.allowRun);
              }
            }}
          />
          <div className="hint">
            사용 중이면 다음 포트로 자동으로 밀립니다. 위의 [상태] 에 실제 주소가 표시됩니다.
          </div>
        </div>

        <h3 style={{ margin: "18px 0 4px", fontSize: 14 }}>클라이언트에 넣을 값</h3>
        <div className="hint" style={{ marginBottom: 8 }}>
          ChatGPT 데스크톱 앱의 [맞춤형 MCP에 연결] 화면은 칸이 따로 있습니다. 아래 세 값을
          해당 칸에 하나씩 붙여 넣으세요. 유형은 <strong>스트리밍 가능한 HTTP</strong> 입니다.
        </div>

        {[
          { id: "mcp-url", label: "URL", value: mcp?.connectUrl ?? "" },
          { id: "mcp-header-name", label: "헤더 키", value: mcp?.headerName ?? "" },
          { id: "mcp-header-value", label: "헤더 값", value: mcp?.headerValue ?? "" },
        ].map((row) => (
          <div className="field" key={row.id}>
            <label htmlFor={row.id}>{row.label}</label>
            <div style={{ display: "flex", gap: 6 }}>
              <input
                id={row.id}
                type="text"
                readOnly
                className="mono"
                style={{ flex: 1 }}
                value={row.value}
                onFocus={(e) => e.currentTarget.select()}
              />
              <button onClick={() => void copyValue(row.label, row.value, row.id)} disabled={!mcp}>
                복사
              </button>
            </div>
          </div>
        ))}

        <div className="hint" style={{ marginBottom: 10 }}>
          [헤더 값] 에는 토큰이 들어 있습니다. 다른 사람에게 그대로 보내지 마세요.
          ChatGPT 폼의 <span className="mono">기본 token 환경 변수</span> 칸은 비워 둡니다 —
          거기는 토큰이 아니라 환경 변수의 <em>이름</em>을 받는 자리입니다.
        </div>

        <details style={{ marginBottom: 12 }}>
          <summary style={{ cursor: "pointer", fontSize: 14 }}>
            설정 파일을 쓰는 클라이언트라면 (Codex CLI 등)
          </summary>
          <textarea
            id="mcp-config"
            readOnly
            rows={4}
            className="mono"
            style={{ width: "100%", resize: "vertical", marginTop: 8 }}
            value={mcp?.clientConfig ?? ""}
            onFocus={(e) => e.currentTarget.select()}
          />
          <button
            onClick={() => void copyValue("설정", mcp?.clientConfig ?? "", "mcp-config")}
            disabled={!mcp}
          >
            복사
          </button>
          <div className="hint">
            <span className="mono">~/.codex/config.toml</span> 에 붙여 넣습니다.
          </div>
        </details>

        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <button onClick={() => void openMcpGuide()} disabled={busy}>
            연결 방법 보기
          </button>
          <button onClick={() => void regenerate()} disabled={!mcp || busy}>
            토큰 재발급
          </button>
        </div>
        <div className="hint" style={{ marginTop: 8 }}>
          ChatGPT 데스크톱 앱에 등록하는 방법을 그림과 함께 설명합니다. 등록했는데 도구가 안
          보인다면 대화창이 <strong>Work</strong> 모드인지부터 확인하세요.
        </div>
      </div>

      <div className="card">
        <h2>제거</h2>
        <p style={{ color: "var(--text-dim)" }}>
          전용 배포판을 통째로 제거합니다. 사용자의 다른 WSL 배포판에는 영향이 없습니다.
        </p>
        <button className="danger" onClick={() => void removeEnv()} disabled={busy}>
          배포판 제거
        </button>
      </div>
    </>
  );
}
