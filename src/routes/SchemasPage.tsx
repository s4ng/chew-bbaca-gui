import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

import { formatBytes, formatTime } from "../lib/format";
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

    const suggested = dir.split(/[\\/]/).filter(Boolean).pop() ?? "가져온 스키마";
    const name = window.prompt(
      "이 스키마를 무엇으로 부를까요?\n(목록에 표시될 이름입니다)",
      suggested,
    );
    if (name === null) return;

    setImporting(true);
    setError(null);
    setMessage(null);
    try {
      const info = await schemasImport(dir, name);
      setMessage(
        `'${info.name}' 를 가져왔습니다${info.lociCount ? ` (loci ${info.lociCount})` : ""}.`,
      );
      await refresh();
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setImporting(false);
    }
  };

  const remove = async (schema: SchemaInfo) => {
    const ok = window.confirm(
      `'${schema.name}' 스키마를 삭제합니다.\n이 스키마로 만든 기존 결과는 남지만, 같은 스키마로 이어서 AlleleCall 을 할 수 없게 됩니다.\n되돌릴 수 없습니다. 계속할까요?`,
    );
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
          <h1>스키마</h1>
          <p>
            스키마는 앱이 소유하며 WSL 내부에 저장됩니다. AlleleCall 이 신규 allele 을 계속
            추가하기 때문에, Windows 폴더에 두면 실행할 때마다 파일시스템 오버헤드가 쌓입니다.
          </p>
        </div>
        <div className="row">
          <button disabled={importing} onClick={() => void importSchema()}>
            {importing ? "가져오는 중..." : "불러오기"}
          </button>
          <button onClick={() => void refresh()}>새로 고침</button>
        </div>
      </div>

      {error && <div className="banner error">{error}</div>}
      {message && <div className="banner info">{message}</div>}

      {schemas.length === 0 ? (
        <div className="empty">
          <p>아직 스키마가 없습니다. [새 작업] → CreateSchema 로 만들 수 있습니다.</p>
          <p style={{ color: "var(--text-dim)", fontSize: 13 }}>
            전에 [내보내기] 로 빼둔 폴더가 있다면 [불러오기] 로 되돌릴 수 있습니다.
          </p>
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
                      ? "내보내는 중..."
                      : "내보내기"}
                  </button>
                  <button
                    className="danger"
                    disabled={busy?.id === s.schemaId}
                    onClick={() => void remove(s)}
                  >
                    {busy?.id === s.schemaId && busy.action === "delete" ? "삭제 중..." : "삭제"}
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
    const ok = window.confirm(
      `training file '${f.name}' 를 삭제합니다.\n이미 이것으로 만든 스키마는 자기 안에 사본을 갖고 있어 영향받지 않습니다.\n되돌릴 수 없습니다. 계속할까요?`,
    );
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
      <h2>Prodigal training file</h2>
      <p style={{ color: "var(--text-dim)", fontSize: 13 }}>
        스키마를 만들 때 쓰는 종별 학습 파일입니다. [새 작업] → CreateSchema 의 training file
        칸에서 게놈 폴더를 고르면 만들 수 있습니다. chewBBACA 가 배포하는 것은 19개 종뿐이라,
        그 밖의 종은 직접 만들어야 합니다.
      </p>

      {files.length === 0 ? (
        <div className="empty">
          <p>아직 training file 이 없습니다.</p>
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
                  삭제
                </button>
              </div>
              <table className="kv" style={{ marginTop: 8 }}>
                <tbody>
                  <tr>
                    <td>만든 날짜</td>
                    <td>{formatTime(f.createdAt)}</td>
                  </tr>
                  <tr>
                    <td>크기</td>
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
