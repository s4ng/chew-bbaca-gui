import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

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
      const now = window.confirm(
        `데이터 폴더를 여기로 바꿨습니다:\n${root}\n\n` +
          "적용하려면 앱을 다시 시작해야 합니다. 지금 다시 시작할까요?\n" +
          "[취소] 를 눌러도 설정은 남아 다음 실행부터 적용됩니다.",
      );
      if (now) {
        await appRestart();
        return;
      }
      setMessage(`다음 실행부터 ${root} 를 씁니다.`);
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

    const ok = window.confirm(
      `${picked}\n\n` +
        "이 폴더 안에 ChewieApp 폴더를 만들어 데이터 폴더로 씁니다.\n" +
        "여기에 수 GB 짜리 가상 디스크가 만들어지고, 앱을 제거할 때 이 폴더는 통째로 지워집니다.\n" +
        "계속할까요?",
    );
    if (!ok) return;
    await change(picked);
  };

  const reset = async () => {
    if (!info) return;
    const ok = window.confirm(
      `데이터 폴더를 기본 위치로 되돌립니다:\n${info.defaultDir}\n\n` +
        "지금 폴더에 있는 파일은 옮기지 않습니다. 계속할까요?",
    );
    if (!ok) return;
    await change(info.defaultDir);
  };

  if (!info) return null;

  return (
    <div className="field">
      <label>데이터 폴더</label>
      <div className="row">
        <input className="path" value={info.current} readOnly />
        {info.changeable && (
          <>
            <button onClick={() => void pick()} disabled={busy}>
              변경
            </button>
            {!info.isDefault && (
              <button onClick={() => void reset()} disabled={busy}>
                기본 위치로
              </button>
            )}
          </>
        )}
      </div>

      {error && <div className="banner error">{error}</div>}
      {message && <div className="banner info">{message}</div>}

      {info.changeable ? (
        <div className="hint">
          가상 디스크(<code>ext4.vhdx</code>)가 이 폴더에 만들어지고 분석을 돌릴수록 수 GB 까지
          자랍니다. C 드라이브가 좁다면 <strong>설치 전에</strong> 다른 내장 드라이브로 바꿔
          두세요. 이동식·네트워크 드라이브와 exFAT 은 쓸 수 없습니다.
        </div>
      ) : (
        <div className="hint">{info.reason}</div>
      )}
    </div>
  );
}
