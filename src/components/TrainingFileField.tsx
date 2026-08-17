import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { formatBases } from "../lib/format";
import { useT } from "../lib/i18n";
import { trainingCreate, trainingList, trainingScan } from "../lib/ipc";
import { asAppError, type GenomeScan, type TrainingFile } from "../lib/types";

/**
 * Prodigal training file(`.trn`) 을 고르거나 만드는 칸.
 *
 * CreateSchema 와 PrepExternalSchema 가 같은 것을 쓰므로 컴포넌트로 뺐다.
 *
 * **세 가지 경로를 한 자리에 둔다** — 저장소에 이미 있는 것 고르기, 게놈 폴더에서
 * 새로 만들기, 외부에서 받은 `.trn` 파일 직접 지정하기. 마지막 것을 남겨두는
 * 이유는 chewBBACA 가 19개 종의 training file 만 배포하고, 그 밖의 종을 다루는
 * 사용자는 어디선가 받은 파일을 그대로 쓰는 경우가 흔하기 때문이다.
 */
export default function TrainingFileField({
  id,
  value,
  onChange,
  hint,
}: {
  id: string;
  value: string;
  onChange: (v: string) => void;
  hint: React.ReactNode;
}) {
  const t = useT();
  const [files, setFiles] = useState<TrainingFile[]>([]);
  const [panelOpen, setOpen] = useState(false);
  const [genomeDir, setGenomeDir] = useState("");
  const [scan, setScan] = useState<GenomeScan | null>(null);
  const [picked, setPicked] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState<null | "scan" | "create">(null);
  const [error, setError] = useState<string | null>(null);

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

  /** 저장소 밖의 파일을 쓰고 있으면 목록에 없으므로 select 가 값을 잃는다. */
  const isExternal = value !== "" && !files.some((f) => f.path === value);

  const pickGenomeDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== "string") return;

    setGenomeDir(dir);
    setScan(null);
    setPicked("");
    setError(null);
    setBusy("scan");
    try {
      const result = await trainingScan(dir);
      setScan(result);
      setPicked(result.candidates[0]?.path ?? "");
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(null);
    }
  };

  const create = async () => {
    setError(null);
    setBusy("create");
    try {
      const created = await trainingCreate(name, genomeDir, picked || null);
      await refresh();
      onChange(created.file.path);
      setOpen(false);
      setGenomeDir("");
      setScan(null);
      setName("");
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="field">
      <label htmlFor={id}>{t.training.label}</label>

      <div className="row">
        <select id={id} value={value} onChange={(e) => onChange(e.target.value)}>
          <option value="">{t.common.none}</option>
          {files.map((f) => (
            <option key={f.path} value={f.path}>
              {f.name}
            </option>
          ))}
          {isExternal && <option value={value}>{value}</option>}
        </select>
        <button onClick={() => setOpen((v) => !v)} disabled={busy !== null}>
          {panelOpen ? t.common.close : t.training.createFromDir}
        </button>
        <button
          onClick={async () => {
            const p = await open({
              multiple: false,
              filters: [{ name: t.training.fileFilter, extensions: ["trn"] }],
            });
            if (typeof p === "string") onChange(p);
          }}
        >
          {t.training.pickFile}
        </button>
      </div>

      {panelOpen && (
        <div className="module-info" style={{ marginTop: 8 }}>
          <p className="summary">{t.training.intro}</p>

          <div className="row">
            <input
              type="text"
              value={genomeDir}
              readOnly
              placeholder={t.training.genomeDirPlaceholder}
            />
            <button onClick={() => void pickGenomeDir()} disabled={busy !== null}>
              {busy === "scan" ? t.training.scanning : t.training.pickDir}
            </button>
          </div>

          {busy === "scan" && <div className="hint">{t.training.scanningHint}</div>}

          {scan && (
            <>
              <div className="hint">{scan.reason}</div>

              <div className="field" style={{ marginTop: 8 }}>
                <label htmlFor={`${id}-genome`}>{t.training.genomeField}</label>
                <select
                  id={`${id}-genome`}
                  value={picked}
                  onChange={(e) => setPicked(e.target.value)}
                >
                  {scan.candidates.map((c, i) => (
                    <option key={c.path} value={c.path}>
                      {i === 0 ? "★ " : ""}
                      {t.training.candidate(c.fileName, c.contigs, formatBases(c.bases))}
                    </option>
                  ))}
                </select>
                <div className="hint">{t.training.candidateHint(scan.scanned)}</div>
              </div>

              <div className="field">
                <label htmlFor={`${id}-name`}>{t.training.nameField}</label>
                <input
                  id={`${id}-name`}
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder={t.training.namePlaceholder}
                />
                <div className="hint">{t.training.nameHint}</div>
              </div>

              <div className="row">
                <button
                  className="primary"
                  onClick={() => void create()}
                  disabled={busy !== null || !name.trim() || !picked}
                >
                  {busy === "create" ? t.training.creating : t.training.create}
                </button>
                <button onClick={() => setOpen(false)} disabled={busy !== null}>
                  {t.common.cancel}
                </button>
              </div>
            </>
          )}

          {error && (
            <div className="banner error" style={{ marginTop: 8, marginBottom: 0 }}>
              {error}
            </div>
          )}
        </div>
      )}

      {value === "" ? (
        // chewBBACA 는 CLI 마지막 줄에서 training file 을 넣으라고 권고하는데,
        // GUI 사용자는 그 줄을 볼 일이 없다. 비워둘 때 무엇을 감수하는지 여기서 말한다.
        <div className="hint">{t.training.emptyHint}</div>
      ) : (
        <div className="hint">{hint}</div>
      )}
    </div>
  );
}

