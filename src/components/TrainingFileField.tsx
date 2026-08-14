import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

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
      <label htmlFor={id}>Prodigal training file (.trn) — 선택</label>

      <div className="row">
        <select id={id} value={value} onChange={(e) => onChange(e.target.value)}>
          <option value="">(없음)</option>
          {files.map((f) => (
            <option key={f.path} value={f.path}>
              {f.name}
            </option>
          ))}
          {isExternal && <option value={value}>{value}</option>}
        </select>
        <button onClick={() => setOpen((v) => !v)} disabled={busy !== null}>
          {panelOpen ? "닫기" : "폴더에서 만들기"}
        </button>
        <button
          onClick={async () => {
            const p = await open({
              multiple: false,
              filters: [{ name: "Prodigal training file", extensions: ["trn"] }],
            });
            if (typeof p === "string") onChange(p);
          }}
        >
          파일에서 고르기
        </button>
      </div>

      {panelOpen && (
        <div className="module-info" style={{ marginTop: 8 }}>
          <p className="summary">
            그 종의 게놈이 든 폴더를 고르면, <b>contig 가 가장 적은 어셈블리 하나</b>를 골라
            학습시킵니다. 폴더 전체를 쓰지 않는 이유는 게놈 하나면 통계가 수렴하는 반면,
            섞여 있는 저품질 어셈블리는 모델을 조용히 나쁘게 만들기 때문입니다.
          </p>

          <div className="row">
            <input type="text" value={genomeDir} readOnly placeholder="게놈 FASTA 가 든 폴더" />
            <button onClick={() => void pickGenomeDir()} disabled={busy !== null}>
              {busy === "scan" ? "훑는 중…" : "폴더 선택"}
            </button>
          </div>

          {busy === "scan" && (
            <div className="hint">
              폴더의 FASTA 를 모두 읽고 있습니다. contig 수는 파일 크기로 알 수 없어 전부 읽어야
              합니다 — 게놈 수백 개면 몇 초 걸립니다.
            </div>
          )}

          {scan && (
            <>
              <div className="hint">{scan.reason}</div>

              <div className="field" style={{ marginTop: 8 }}>
                <label htmlFor={`${id}-genome`}>학습에 쓸 게놈</label>
                <select
                  id={`${id}-genome`}
                  value={picked}
                  onChange={(e) => setPicked(e.target.value)}
                >
                  {scan.candidates.map((c, i) => (
                    <option key={c.path} value={c.path}>
                      {i === 0 ? "★ " : ""}
                      {c.fileName} — contig {c.contigs}개, {formatBases(c.bases)}
                    </option>
                  ))}
                </select>
                <div className="hint">
                  FASTA {scan.scanned}개 중 크기가 정상 범위인 것만 후보로 올렸습니다. 보통은 맨 위
                  것을 그대로 쓰면 됩니다.
                </div>
              </div>

              <div className="field">
                <label htmlFor={`${id}-name`}>이름</label>
                <input
                  id={`${id}-name`}
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="예: B_fragilis"
                />
                <div className="hint">
                  확장자는 빼고 적습니다. 같은 이름이 이미 있으면 덮어쓰지 않고 실패합니다.
                </div>
              </div>

              <div className="row">
                <button
                  className="primary"
                  onClick={() => void create()}
                  disabled={busy !== null || !name.trim() || !picked}
                >
                  {busy === "create" ? "학습 중… (수십 초)" : "만들기"}
                </button>
                <button onClick={() => setOpen(false)} disabled={busy !== null}>
                  취소
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
        <div className="hint">
          비워두면 게놈마다 따로 학습해 CDS 경계가 조금씩 달라지고, 불필요한 신규 allele 이
          늘어납니다. 다른 곳의 결과와 합칠 계획이라면 넣는 것을 권합니다.
        </div>
      ) : (
        <div className="hint">{hint}</div>
      )}
    </div>
  );
}

/** 세균 게놈은 Mb 단위다. Rust `fasta::human_bases` 와 같은 규칙. */
function formatBases(bases: number): string {
  return bases >= 1_000_000
    ? `${(bases / 1_000_000).toFixed(2)} Mb`
    : `${Math.round(bases / 1000)} kb`;
}
