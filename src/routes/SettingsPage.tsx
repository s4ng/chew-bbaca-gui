import { useEffect, useState } from "react";

import DataDirField from "../components/DataDirField";
import { formatBytes } from "../lib/format";
import { useLangSetting, useT, type LangSetting } from "../lib/i18n";
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
  workPrunable,
  workPrune,
} from "../lib/ipc";
import {
  asAppError,
  type BackendStatus,
  type DiskUsage,
  type McpStatus,
  type Settings,
  type WorkDirEntry,
} from "../lib/types";

/// 결과를 회수하지 않은 채 완료된 작업. 백엔드 폴더가 **유일한 사본**일 수 있으므로
/// 기본으로 선택하지 않는다 — 0.4.2 이전에 복구된 작업이 정확히 이 상태다.
const isOnlyCopy = (e: WorkDirEntry) => e.status === "completed" && e.outputPath == null;

export default function SettingsPage({ onEnvChanged }: { onEnvChanged: () => Promise<void> | void }) {
  const t = useT();
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
  /** null = 아직 훑어보지 않음. `du` 가 수 초 걸릴 수 있어 버튼을 눌러야 센다. */
  const [prunable, setPrunable] = useState<WorkDirEntry[] | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  /**
   * 디스크 카드의 결과는 **카드 안에서** 보여준다.
   *
   * 페이지 최상단 배너로 보내면 안 된다 — 디스크 카드는 세 번째 카드라 버튼을 누르는
   * 시점에 배너가 스크롤 밖에 있고, 사용자에게는 눌러도 아무 일이 없는 것으로 보인다.
   * 실제로 그렇게 신고가 들어왔다.
   */
  const [diskMsg, setDiskMsg] = useState<string | null>(null);
  const [diskError, setDiskError] = useState<string | null>(null);
  /** 도는 동안 버튼 라벨을 바꾼다. `disk_compact` 는 배포판 종료를 포함해 수 초 걸린다. */
  const [diskAction, setDiskAction] = useState<"scan" | "prune" | "compact" | null>(null);

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
    const ok = window.confirm(t.settings.mcpRegenerateConfirm);
    if (!ok) return;
    setBusy(true);
    try {
      setMcp(await mcpRegenerateToken());
      setMessage(t.settings.mcpRegenerated);
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
      setMessage(t.settings.mcpCopied(label));
    } catch {
      // 웹뷰가 보안 컨텍스트가 아니면 클립보드 API 가 없다. 그때는 직접 고르게 한다.
      const el = document.getElementById(elementId) as
        | HTMLInputElement
        | HTMLTextAreaElement
        | null;
      el?.select();
      setMessage(t.settings.mcpCopyFailed);
    }
  };

  const save = async (next: Settings) => {
    setSettings(next);
    try {
      await settingsSet(next);
      setMessage(t.settings.saved);
    } catch (e) {
      setError(asAppError(e).message);
    }
  };

  const loadPrunable = async () => {
    setBusy(true);
    setDiskAction("scan");
    setDiskMsg(null);
    setDiskError(null);
    try {
      const list = await workPrunable();
      setPrunable(list);
      // 유일한 사본일 수 있는 것은 사용자가 직접 켜야 지워진다.
      setPicked(new Set(list.filter((e) => !isOnlyCopy(e)).map((e) => e.jobId)));
      const total = list.reduce((sum, e) => sum + e.bytes, 0);
      setDiskMsg(
        list.length === 0
          ? t.settings.scanEmpty
          : t.settings.scanFound(list.length, formatBytes(total)),
      );
    } catch (e) {
      setDiskError(asAppError(e).message);
    } finally {
      setDiskAction(null);
      setBusy(false);
    }
  };

  const prune = async () => {
    const targets = (prunable ?? []).filter((e) => picked.has(e.jobId));
    if (targets.length === 0) return;
    const risky = targets.filter(isOnlyCopy).length;
    const ok = window.confirm(
      t.settings.confirmPrune(
        targets.length,
        formatBytes(targets.reduce((sum, e) => sum + e.bytes, 0)),
        risky,
      ),
    );
    if (!ok) return;
    setBusy(true);
    setDiskAction("prune");
    setDiskMsg(null);
    setDiskError(null);
    try {
      const result = await workPrune(targets.map((e) => e.jobId));
      setDiskMsg(t.settings.pruned(result.removed, formatBytes(result.freedBytes)));
      setPrunable(await workPrunable().catch(() => []));
      setPicked(new Set());
      setDisk(await diskUsage());
    } catch (e) {
      setDiskError(asAppError(e).message);
    } finally {
      setDiskAction(null);
      setBusy(false);
    }
  };

  /**
   * sparse 전환은 **파일을 즉시 줄이지 않는다.** 앞으로 배포판이 반납하는 블록을
   * Windows 가 회수할 수 있게 표시하는 것이라, 대부분의 경우 방금 누른 직후의
   * 크기는 그대로다. 그 사실을 말해주지 않으면 "정리했다는데 숫자가 그대로" 가 된다.
   */
  const compact = async () => {
    setBusy(true);
    setDiskAction("compact");
    setDiskMsg(null);
    setDiskError(null);
    const before = disk?.vhdxBytes ?? null;
    try {
      const note = await diskCompact();
      const next = await diskUsage();
      setDisk(next);
      const after = next.vhdxBytes ?? null;
      const freed = before != null && after != null ? before - after : null;
      setDiskMsg(
        freed != null && freed > 0
          ? t.settings.compactedFreed(note, formatBytes(freed), formatBytes(after))
          : t.settings.compactedSame(note, formatBytes(after)),
      );
    } catch (e) {
      setDiskError(asAppError(e).message);
    } finally {
      setDiskAction(null);
      setBusy(false);
    }
  };

  const removeEnv = async () => {
    const ok = window.confirm(t.settings.removeEnvConfirm);
    if (!ok) return;
    setBusy(true);
    try {
      await envUnregister();
      setMessage(t.settings.removedEnv);
      await onEnvChanged();
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(false);
    }
  };

  if (!settings) {
    return <div className="empty">{t.settings.loading}</div>;
  }

  return (
    <>
      <div className="page-head">
        <div>
          <h1>{t.settings.title}</h1>
          <p>{t.settings.subtitle}</p>
        </div>
      </div>

      {error && <div className="banner error">{error}</div>}
      {message && <div className="banner info">{message}</div>}

      <LanguageCard />

      <div className="card">
        <h2>{t.settings.envTitle}</h2>
        <table className="kv">
          <tbody>
            <tr>
              <td>{t.settings.distro}</td>
              <td className="mono">{settings.distro}</td>
            </tr>
            <tr>
              <td>{t.settings.chewbbaca}</td>
              <td>{backend?.chewbbacaVersion ?? t.settings.unknown}</td>
            </tr>
            <tr>
              <td>{t.settings.cpuCount}</td>
              <td>{backend?.cpuCount ?? t.common.dash}</td>
            </tr>
            <tr>
              <td>{t.settings.state}</td>
              <td>{backend?.detail ?? t.common.dash}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <div className="card">
        <h2>{t.settings.runTitle}</h2>
        <div className="field">
          <label htmlFor="cpu">{t.settings.defaultCpu}</label>
          <input
            id="cpu"
            type="number"
            min={1}
            value={settings.defaultCpu ?? ""}
            placeholder={t.settings.defaultCpuPlaceholder}
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
          {t.settings.keepWorkDir}
        </label>
      </div>

      <div className="card">
        <h2>{t.settings.diskTitle}</h2>
        <p style={{ color: "var(--text-dim)" }}>{t.settings.diskIntro}</p>
        <table className="kv">
          <tbody>
            <tr>
              <td>{t.settings.vhdx}</td>
              <td>{formatBytes(disk?.vhdxBytes ?? null)}</td>
            </tr>
          </tbody>
        </table>
        <DataDirField onChanged={() => void diskUsage().then(setDisk).catch(() => undefined)} />
        <p style={{ color: "var(--text-dim)" }}>{t.settings.pruneIntro}</p>
        <div className="row" style={{ marginTop: 10 }}>
          <button onClick={() => void loadPrunable()} disabled={busy}>
            {diskAction === "scan" ? t.settings.scanning : t.settings.scan}
          </button>
          <button onClick={() => void compact()} disabled={busy}>
            {diskAction === "compact" ? t.settings.compacting : t.settings.compact}
          </button>
        </div>

        {/* 결과는 누른 버튼 바로 아래에 남는다 — 페이지 최상단 배너는 여기서 안 보인다. */}
        {diskError && (
          <div className="banner error" style={{ marginTop: 10 }}>
            {diskError}
          </div>
        )}
        {diskMsg && (
          <div className="banner info" style={{ marginTop: 10 }}>
            {diskMsg}
          </div>
        )}

        {prunable != null && prunable.length > 0 && (
          <>
            <table className="kv" style={{ marginTop: 12 }}>
              <tbody>
                {prunable.map((e) => (
                  <tr key={e.jobId}>
                    <td>
                      <label className="inline-check" style={{ margin: 0 }}>
                        <input
                          type="checkbox"
                          checked={picked.has(e.jobId)}
                          onChange={(ev) => {
                            const next = new Set(picked);
                            if (ev.target.checked) next.add(e.jobId);
                            else next.delete(e.jobId);
                            setPicked(next);
                          }}
                        />
                        {t.module[e.module]}{" "}
                        <span className={`pill ${e.status}`}>{t.status[e.status]}</span>
                      </label>
                      {isOnlyCopy(e) && (
                        <div style={{ color: "var(--warn)", fontSize: "0.9em" }}>
                          {t.settings.onlyCopy}
                        </div>
                      )}
                    </td>
                    <td>{formatBytes(e.bytes)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <button
              className="danger"
              onClick={() => void prune()}
              disabled={busy || picked.size === 0}
              style={{ marginTop: 10 }}
            >
              {diskAction === "prune"
                ? t.settings.pruning
                : t.settings.pruneButton(
                    picked.size,
                    formatBytes(
                      prunable
                        .filter((e) => picked.has(e.jobId))
                        .reduce((sum, e) => sum + e.bytes, 0),
                    ),
                  )}
            </button>
          </>
        )}
      </div>

      <div className="card">
        <h2>{t.settings.rootfsTitle}</h2>
        <p style={{ color: "var(--text-dim)" }}>{t.settings.rootfsIntro}</p>
        <div className="field">
          <label htmlFor="url">{t.settings.rootfsUrl}</label>
          <input
            id="url"
            type="text"
            value={settings.rootfs.url}
            onChange={(e) =>
              setSettings({ ...settings, rootfs: { ...settings.rootfs, url: e.target.value } })
            }
            onBlur={() => void save(settings)}
          />
          <div className="hint">{t.settings.rootfsUrlHint}</div>
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
          <div className="hint">{t.settings.rootfsShaHint}</div>
        </div>
      </div>

      <div className="card">
        <h2>{t.settings.mcpTitle}</h2>
        <p style={{ color: "var(--text-dim)" }}>{t.settings.mcpIntro}</p>
        <table className="kv">
          <tbody>
            <tr>
              <td>{t.settings.state}</td>
              <td>
                {!mcp
                  ? t.settings.mcpChecking
                  : mcp.running
                    ? t.settings.mcpRunning(mcp.url ?? "")
                    : mcp.enabled
                      ? t.settings.mcpFailed
                      : t.settings.mcpOff}
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
          {t.settings.mcpEnable}
        </label>
        <label className="inline-check">
          <input
            type="checkbox"
            checked={mcp?.allowRun ?? false}
            disabled={!mcp || busy}
            onChange={(e) => void applyMcp(mcp?.enabled ?? true, port, e.target.checked)}
          />
          {t.settings.mcpAllowRun}
        </label>
        <div className="hint" style={{ marginBottom: 10 }}>
          {t.settings.mcpAllowRunHint}
        </div>

        <div className="field">
          <label htmlFor="mcp-port">{t.settings.mcpPort}</label>
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
          <div className="hint">{t.settings.mcpPortHint}</div>
        </div>

        <h3 style={{ margin: "18px 0 4px", fontSize: 14 }}>{t.settings.mcpClientValues}</h3>
        <div className="hint" style={{ marginBottom: 8 }}>
          {t.settings.mcpClientValuesHint}
        </div>

        {[
          { id: "mcp-url", label: "URL", value: mcp?.connectUrl ?? "" },
          { id: "mcp-header-name", label: t.settings.mcpHeaderName, value: mcp?.headerName ?? "" },
          {
            id: "mcp-header-value",
            label: t.settings.mcpHeaderValue,
            value: mcp?.headerValue ?? "",
          },
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
                {t.settings.mcpCopy}
              </button>
            </div>
          </div>
        ))}

        <div className="hint" style={{ marginBottom: 10 }}>
          {t.settings.mcpTokenWarning}
        </div>

        <details style={{ marginBottom: 12 }}>
          <summary style={{ cursor: "pointer", fontSize: 14 }}>
            {t.settings.mcpConfigSummary}
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
            onClick={() =>
              void copyValue(t.settings.mcpConfigLabel, mcp?.clientConfig ?? "", "mcp-config")
            }
            disabled={!mcp}
          >
            {t.settings.mcpCopy}
          </button>
          <div className="hint">{t.settings.mcpConfigHint}</div>
        </details>

        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <button onClick={() => void openMcpGuide()} disabled={busy}>
            {t.settings.mcpOpenGuide}
          </button>
          <button onClick={() => void regenerate()} disabled={!mcp || busy}>
            {t.settings.mcpRegenerate}
          </button>
        </div>
        <div className="hint" style={{ marginTop: 8 }}>
          {t.settings.mcpGuideHint}
        </div>
      </div>

      <div className="card">
        <h2>{t.settings.removeTitle}</h2>
        <p style={{ color: "var(--text-dim)" }}>{t.settings.removeIntro}</p>
        <button className="danger" onClick={() => void removeEnv()} disabled={busy}>
          {t.settings.removeEnv}
        </button>
      </div>
    </>
  );
}

/**
 * 표시 언어 선택.
 *
 * 맨 위에 두는 이유는 단순하다 — 언어를 잘못 만난 사용자가 **읽지 못하는 화면을
 * 스크롤하지 않고** 바꿀 수 있어야 한다. 그래서 이 카드만 제목도 두 언어를 함께
 * 적고, 선택지는 각 언어의 자기 이름(한국어 / English)으로 둔다.
 */
function LanguageCard() {
  const t = useT();
  const { lang, setting, setSetting } = useLangSetting();

  return (
    <div className="card">
      <h2>{t.lang.title}</h2>
      <div className="field">
        <label htmlFor="lang">{t.lang.label}</label>
        <select
          id="lang"
          value={setting}
          onChange={(e) => setSetting(e.target.value as LangSetting)}
        >
          <option value="auto">{t.lang.auto}</option>
          <option value="ko">{t.lang.ko}</option>
          <option value="en">{t.lang.en}</option>
        </select>
        <div className="hint">
          {setting === "auto" ? `${t.lang.autoResolved(t.lang[lang])} ` : ""}
          {t.lang.backendNote}
        </div>
      </div>
    </div>
  );
}
