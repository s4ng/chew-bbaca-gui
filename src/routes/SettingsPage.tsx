import { useEffect, useState } from "react";

import { formatBytes } from "../lib/format";
import {
  backendStatus,
  diskCompact,
  diskUsage,
  envUnregister,
  settingsGet,
  settingsSet,
} from "../lib/ipc";
import { asAppError, type BackendStatus, type DiskUsage, type Settings } from "../lib/types";

export default function SettingsPage({ onEnvChanged }: { onEnvChanged: () => Promise<void> | void }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [backend, setBackend] = useState<BackendStatus | null>(null);
  const [disk, setDisk] = useState<DiskUsage | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void settingsGet().then(setSettings).catch((e) => setError(asAppError(e).message));
    void backendStatus().then(setBackend).catch(() => setBackend(null));
    void diskUsage().then(setDisk).catch(() => setDisk(null));
  }, []);

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
        <h2>rootfs 배포 정보</h2>
        <p style={{ color: "var(--text-dim)" }}>
          첫 실행 시 내려받는 이미지의 위치와 체크섬입니다. 정식 릴리스에서는 앱에 기본값이
          들어 있으며, 직접 빌드한 rootfs 를 쓸 때만 바꾸면 됩니다.
        </p>
        <div className="field">
          <label htmlFor="url">URL</label>
          <input
            id="url"
            type="text"
            value={settings.rootfs.url}
            onChange={(e) =>
              setSettings({ ...settings, rootfs: { ...settings.rootfs, url: e.target.value } })
            }
            onBlur={() => void save(settings)}
          />
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
