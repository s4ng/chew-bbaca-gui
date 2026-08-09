import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

import { formatTime } from "../lib/format";
import { schemasDelete, schemasExport, schemasList } from "../lib/ipc";
import { asAppError, type SchemaInfo } from "../lib/types";

export default function SchemasPage() {
  const [schemas, setSchemas] = useState<SchemaInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setSchemas(await schemasList());
      setError(null);
    } catch (e) {
      setError(asAppError(e).message);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const exportSchema = async (schema: SchemaInfo) => {
    const dest = await open({ directory: true, multiple: false });
    if (typeof dest !== "string") return;
    setBusy(schema.schemaId);
    try {
      const written = await schemasExport(schema.schemaId, dest);
      await revealItemInDir(written);
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(null);
    }
  };

  const remove = async (schema: SchemaInfo) => {
    const ok = window.confirm(
      `'${schema.name}' 스키마를 삭제합니다.\n이 스키마로 만든 기존 결과는 남지만, 같은 스키마로 이어서 AlleleCall 을 할 수 없게 됩니다.\n되돌릴 수 없습니다. 계속할까요?`,
    );
    if (!ok) return;
    setBusy(schema.schemaId);
    try {
      await schemasDelete(schema.schemaId);
      await refresh();
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(null);
    }
  };

  return (
    <>
      <div className="page-head">
        <div>
          <h1>스키마</h1>
          <p>
            스키마는 앱이 소유하며 WSL 내부에 저장됩니다. AlleleCall 이 신규 allele 을 계속
            추가하기 때문에, Windows 폴더에 두면 실행할 때마다 파일시스템 오버헤드가 쌓입니다.
          </p>
        </div>
        <button onClick={() => void refresh()}>새로 고침</button>
      </div>

      {error && <div className="banner error">{error}</div>}

      {schemas.length === 0 ? (
        <div className="empty">
          <p>아직 스키마가 없습니다. [새 작업] → CreateSchema 로 만들 수 있습니다.</p>
        </div>
      ) : (
        <div className="stack">
          {schemas.map((s) => (
            <div key={s.schemaId} className="card tight">
              <div className="row spread">
                <div>
                  <strong>{s.name}</strong>
                  <div className="path">{s.schemaId}</div>
                </div>
                <div className="row">
                  <button disabled={busy === s.schemaId} onClick={() => void exportSchema(s)}>
                    내보내기
                  </button>
                  <button
                    className="danger"
                    disabled={busy === s.schemaId}
                    onClick={() => void remove(s)}
                  >
                    삭제
                  </button>
                </div>
              </div>
              <table className="kv" style={{ marginTop: 8 }}>
                <tbody>
                  <tr>
                    <td>생성</td>
                    <td>{formatTime(s.createdAt)}</td>
                  </tr>
                  <tr>
                    <td>loci 수</td>
                    <td>{s.lociCount ?? "—"}</td>
                  </tr>
                  <tr>
                    <td>training file</td>
                    <td className="path">{s.ptf ?? "없음"}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          ))}
        </div>
      )}
    </>
  );
}
