import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { useT } from "../lib/i18n";
import { appRestart, dataDirInfo, dataDirSet } from "../lib/ipc";
import { asAppError, type DataDirInfo } from "../lib/types";

/**
 * 데이터 폴더 위치 (ARCHITECTURE.md §5.3).
 *
 * 온보딩 ③ 과 설정 화면이 같은 것을 보여주므로 컴포넌트로 뺀다. 두 곳에서 하는
 * 말은 다르다 — 온보딩은 **설치 전에** 골라야 한다는 것이 요점이고, 설정은 이미
 * 설치된 사용자에게 왜 지금은 못 옮기는지를 알려주는 것이 요점이다.
 *
 * 이 화면이 존재하는 이유는 하나다: 용량의 실체인 `wsl\ext4.vhdx` 가 수 GB 로
 * 자라는데, C 드라이브가 작은 기기가 흔하다.
 */
export default function DataDirField({ onChanged }: { onChanged?: () => void }) {
  const t = useT();
  const [info, setInfo] = useState<DataDirInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    setInfo(await dataDirInfo().catch(() => null));
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  /** 폴더를 골라 위치를 바꾸고, 사용자가 원하면 바로 다시 시작한다. */
  const change = async (picked: string) => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const root = await dataDirSet(picked);
      await refresh();
      onChanged?.();

      // 앱을 다시 시작해야 새 경로로 배선된다. 거부해도 설정은 남는다 —
      // 그 사실을 말해주지 않으면 다음 실행에서 폴더가 바뀐 이유를 알 수 없다.
      const now = window.confirm(t.dataDir.confirmRestart(root));
      if (now) {
        await appRestart();
        return;
      }
      setMessage(t.dataDir.appliesNextRun(root));
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const pick = async () => {
    setError(null);
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== "string") return;

    const ok = window.confirm(t.dataDir.confirmPick(picked));
    if (!ok) return;
    await change(picked);
  };

  const reset = async () => {
    if (!info) return;
    const ok = window.confirm(t.dataDir.confirmReset(info.defaultDir));
    if (!ok) return;
    await change(info.defaultDir);
  };

  if (!info) return null;

  return (
    <div className="field">
      <label>{t.dataDir.label}</label>
      <div className="row">
        <input className="path" value={info.current} readOnly />
        {info.changeable && (
          <>
            <button onClick={() => void pick()} disabled={busy}>
              {t.dataDir.change}
            </button>
            {!info.isDefault && (
              <button onClick={() => void reset()} disabled={busy}>
                {t.dataDir.reset}
              </button>
            )}
          </>
        )}
      </div>

      {error && <div className="banner error">{error}</div>}
      {message && <div className="banner info">{message}</div>}

      {/*
        배포판이 이미 있으면 `reason` 은 Rust 가 만든 한국어 문장이다. 백엔드 문자열
        번역은 이번 범위 밖이라, 그때는 그것을 그대로 보여준다 (`lib/i18n.tsx` 참조).
      */}
      <div className="hint">{info.changeable ? t.dataDir.hint : info.reason}</div>
    </div>
  );
}
