import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

import { formatBytes, formatTime } from "../lib/format";
import { useT } from "../lib/i18n";
import {
  schemasDelete,
  schemasExport,
  schemasImport,
  schemasList,
  trainingDelete,
  trainingList,
} from "../lib/ipc";
import { asAppError, type SchemaInfo, type TrainingFile } from "../lib/types";

export default function SchemasPage() {
  const t = useT();
  const [schemas, setSchemas] = useState<SchemaInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  // 어느 스키마가 무엇을 하는 중인지. 내보내기는 loci 수천 개를 옮기느라 몇 분이
  // 걸리므로 버튼이 그저 비활성인 것만으로는 진행 중인지 멈춘 건지 알 수 없다.
  const [busy, setBusy] = useState<{ id: string; action: "export" | "delete" } | null>(null);
  const [importing, setImporting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

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
    setBusy({ id: schema.schemaId, action: "export" });
    try {
      const written = await schemasExport(schema.schemaId, dest);
      await revealItemInDir(written);
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(null);
    }
  };

  /**
   * 내보낸 폴더를 되돌린다. 이름을 따로 묻는 이유는 폴더 이름에서 표시 이름을
   * 복원할 수 없기 때문이다 — 내보내기가 폴더 *내용*만 복사하기 때문.
   */
  const importSchema = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== "string") return;

    const suggested = dir.split(/[\\/]/).filter(Boolean).pop() ?? t.schemas.defaultImportName;
    const name = window.prompt(t.schemas.promptName, suggested);
    if (name === null) return;

    setImporting(true);
    setError(null);
    setMessage(null);
    try {
      const info = await schemasImport(dir, name);
      setMessage(t.schemas.imported(info.name, info.lociCount));
      await refresh();
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setImporting(false);
    }
  };

  const remove = async (schema: SchemaInfo) => {
    const ok = window.confirm(t.schemas.confirmDelete(schema.name));
    if (!ok) return;
    setBusy({ id: schema.schemaId, action: "delete" });
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
          <h1>{t.schemas.title}</h1>
          <p>{t.schemas.subtitle}</p>
        </div>
        <div className="row">
          <button disabled={importing} onClick={() => void importSchema()}>
            {importing ? t.schemas.importing : t.schemas.import}
          </button>
          <button onClick={() => void refresh()}>{t.common.refresh}</button>
        </div>
      </div>

      {error && <div className="banner error">{error}</div>}
      {message && <div className="banner info">{message}</div>}

      {schemas.length === 0 ? (
        <div className="empty">
          <p>{t.schemas.empty}</p>
          <p style={{ color: "var(--text-dim)", fontSize: 13 }}>{t.schemas.emptyHint}</p>
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
                  <button disabled={busy?.id === s.schemaId} onClick={() => void exportSchema(s)}>
                    {busy?.id === s.schemaId && busy.action === "export"
                      ? t.schemas.exporting
                      : t.schemas.export}
                  </button>
                  <button
                    className="danger"
                    disabled={busy?.id === s.schemaId}
                    onClick={() => void remove(s)}
                  >
                    {busy?.id === s.schemaId && busy.action === "delete"
                      ? t.schemas.deleting
                      : t.common.remove}
                  </button>
                </div>
              </div>
              <table className="kv" style={{ marginTop: 8 }}>
                <tbody>
                  <tr>
                    <td>{t.schemas.createdAt}</td>
                    <td>{formatTime(s.createdAt, t)}</td>
                  </tr>
                  <tr>
                    <td>{t.schemas.lociCount}</td>
                    <td>{s.lociCount ?? t.common.dash}</td>
                  </tr>
                  <tr>
                    <td>{t.schemas.trainingFile}</td>
                    <td className="path">{s.ptf ?? t.schemas.noTrainingFile}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          ))}
        </div>
      )}

      <TrainingFiles onError={setError} />
    </>
  );
}

/**
 * training file 저장소 (`%LOCALAPPDATA%\ChewieApp\training\`).
 *
 * 스키마 화면에 붙인 이유는 두 가지다 — `.trn` 은 스키마의 일부처럼 쓰이는
 * 물건이고, 만드는 화면(새 작업)에서 "이미 같은 이름이 있습니다" 로 막혔을 때
 * **지울 수 있는 곳이 어딘가 있어야** 한다.
 */
function TrainingFiles({ onError }: { onError: (m: string | null) => void }) {
  const t = useT();
  const [files, setFiles] = useState<TrainingFile[]>([]);
  const [busy, setBusy] = useState<string | null>(null);

  const refresh = useCallback(
    () =>
      trainingList()
        .then(setFiles)
        .catch(() => setFiles([])),
    [],
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const remove = async (f: TrainingFile) => {
    const ok = window.confirm(t.schemas.confirmDeleteTraining(f.name));
    if (!ok) return;
    setBusy(f.name);
    try {
      await trainingDelete(f.name);
      await refresh();
      onError(null);
    } catch (e) {
      onError(asAppError(e).message);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div style={{ marginTop: 32 }}>
      <h2>{t.schemas.trainingTitle}</h2>
      <p style={{ color: "var(--text-dim)", fontSize: 13 }}>{t.schemas.trainingSubtitle}</p>

      {files.length === 0 ? (
        <div className="empty">
          <p>{t.schemas.trainingEmpty}</p>
        </div>
      ) : (
        <div className="stack">
          {files.map((f) => (
            <div key={f.path} className="card tight">
              <div className="row spread">
                <div>
                  <strong>{f.name}</strong>
                  <div className="path">{f.path}</div>
                </div>
                <button className="danger" disabled={busy === f.name} onClick={() => void remove(f)}>
                  {t.common.remove}
                </button>
              </div>
              <table className="kv" style={{ marginTop: 8 }}>
                <tbody>
                  <tr>
                    <td>{t.schemas.trainingCreatedAt}</td>
                    <td>{formatTime(f.createdAt, t)}</td>
                  </tr>
                  <tr>
                    <td>{t.schemas.trainingSize}</td>
                    <td>{formatBytes(f.sizeBytes)}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
