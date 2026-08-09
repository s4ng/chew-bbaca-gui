import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { backendStatus, inspectInputDir, jobsSubmit, schemasList, settingsGet } from "../lib/ipc";
import {
  asAppError,
  type BackendStatus,
  type InputDirInfo,
  type JobSpec,
  type Module,
  type SchemaInfo,
} from "../lib/types";

export default function NewJobPage({ onSubmitted }: { onSubmitted: () => void }) {
  const [module, setModule] = useState<Module>("CreateSchema");
  const [inputDir, setInputDir] = useState("");
  const [outputDir, setOutputDir] = useState("");
  const [schemaName, setSchemaName] = useState("");
  const [schemaId, setSchemaId] = useState("");
  const [ptf, setPtf] = useState("");
  const [lociList, setLociList] = useState("");
  const [cdsInput, setCdsInput] = useState(false);
  const [cpu, setCpu] = useState("");

  const [schemas, setSchemas] = useState<SchemaInfo[]>([]);
  const [backend, setBackend] = useState<BackendStatus | null>(null);
  const [inputInfo, setInputInfo] = useState<InputDirInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void schemasList().then(setSchemas).catch(() => setSchemas([]));
    void backendStatus().then(setBackend).catch(() => setBackend(null));
    void settingsGet()
      .then((s) => {
        if (s.lastOutputDir) setOutputDir(s.lastOutputDir);
        if (s.defaultCpu) setCpu(String(s.defaultCpu));
      })
      .catch(() => undefined);
  }, []);

  const pickDir = async (setter: (v: string) => void) => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === "string") setter(picked);
  };

  const pickFile = async (setter: (v: string) => void, name: string, extensions: string[]) => {
    const picked = await open({ multiple: false, filters: [{ name, extensions }] });
    if (typeof picked === "string") setter(picked);
  };

  // 입력 폴더를 고르는 즉시 몇 개 파일이 잡히는지 알려준다. 40분 뒤에
  // "빈 폴더였습니다" 를 알게 되는 일이 없어야 한다.
  useEffect(() => {
    if (!inputDir) {
      setInputInfo(null);
      return;
    }
    let cancelled = false;
    inspectInputDir(inputDir)
      .then((info) => !cancelled && setInputInfo(info))
      .catch((e) => {
        if (!cancelled) {
          setInputInfo(null);
          setError(asAppError(e).message);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [inputDir]);

  const submit = async () => {
    setError(null);
    setBusy(true);
    try {
      const spec: JobSpec = {
        module,
        inputDir,
        outputDir,
        cdsInput,
        schemaId: module === "AlleleCall" ? schemaId : null,
        schemaName: module === "CreateSchema" ? schemaName : null,
        ptf: module === "CreateSchema" && ptf ? ptf : null,
        lociList: module === "AlleleCall" && lociList ? lociList : null,
        cpu: cpu ? Number(cpu) : null,
      };
      await jobsSubmit(spec);
      onSubmitted();
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const ready =
    inputDir !== "" &&
    outputDir !== "" &&
    (module === "CreateSchema" ? schemaName.trim() !== "" : schemaId !== "");

  return (
    <>
      <div className="page-head">
        <div>
          <h1>새 작업</h1>
          <p>
            입력 파일은 실행 전에 WSL 내부로 복사됩니다. 원본은 수정되지 않습니다.
          </p>
        </div>
      </div>

      {error && <div className="banner error">{error}</div>}
      {backend && !backend.ready && <div className="banner warn">{backend.detail}</div>}

      <div className="card">
        <div className="field">
          <label htmlFor="module">모듈</label>
          <select
            id="module"
            value={module}
            onChange={(e) => setModule(e.target.value as Module)}
          >
            <option value="CreateSchema">CreateSchema — 어셈블리로부터 wgMLST 스키마 생성</option>
            <option value="AlleleCall">AlleleCall — 균주별 allelic profile 결정</option>
          </select>
        </div>

        <div className="field">
          <label htmlFor="input">어셈블리 폴더</label>
          <div className="row">
            <input id="input" type="text" value={inputDir} readOnly placeholder="폴더를 선택하세요" />
            <button onClick={() => void pickDir(setInputDir)}>찾아보기</button>
          </div>
          <div className="hint">
            {inputInfo
              ? `파일 ${inputInfo.totalFiles}개 (FASTA로 보이는 파일 ${inputInfo.fastaFiles}개)`
              : "네트워크 드라이브(UNC) 경로는 지원하지 않습니다. 로컬 드라이브를 사용하세요."}
          </div>
        </div>

        {module === "CreateSchema" ? (
          <>
            <div className="field">
              <label htmlFor="schemaName">스키마 이름</label>
              <input
                id="schemaName"
                type="text"
                value={schemaName}
                onChange={(e) => setSchemaName(e.target.value)}
                placeholder="예: Listeria monocytogenes 2026-08"
              />
              <div className="hint">
                스키마는 앱이 소유하며 WSL 내부에 저장됩니다. 목록·삭제·내보내기는 [스키마]
                화면에서 할 수 있습니다.
              </div>
            </div>

            <div className="field">
              <label htmlFor="ptf">Prodigal training file (.trn) — 선택</label>
              <div className="row">
                <input id="ptf" type="text" value={ptf} readOnly placeholder="(선택) 종별 .trn 파일" />
                <button onClick={() => void pickFile(setPtf, "Prodigal training file", ["trn"])}>
                  찾아보기
                </button>
                {ptf && <button onClick={() => setPtf("")}>지우기</button>}
              </div>
              <div className="hint">
                지정한 training file 은 스키마 안에 함께 보관되고, 이후 AlleleCall 에서 계속
                같은 것이 쓰입니다. 결과 일관성을 위해 중간에 바꾸지 않습니다.
              </div>
            </div>

            <div className="field">
              <label className="inline-check">
                <input
                  type="checkbox"
                  checked={cdsInput}
                  onChange={(e) => setCdsInput(e.target.checked)}
                />
                입력이 이미 CDS 입니다 (--cds)
              </label>
            </div>
          </>
        ) : (
          <>
            <div className="field">
              <label htmlFor="schema">스키마</label>
              <select id="schema" value={schemaId} onChange={(e) => setSchemaId(e.target.value)}>
                <option value="">선택하세요</option>
                {schemas.map((s) => (
                  <option key={s.schemaId} value={s.schemaId}>
                    {s.name}
                    {s.lociCount ? ` (loci ${s.lociCount})` : ""}
                  </option>
                ))}
              </select>
              {schemas.length === 0 && (
                <div className="hint">
                  아직 스키마가 없습니다. 먼저 CreateSchema 로 스키마를 만드세요.
                </div>
              )}
            </div>

            <div className="field">
              <label htmlFor="gl">일부 loci 만 대상으로 (--gl) — 선택</label>
              <div className="row">
                <input id="gl" type="text" value={lociList} readOnly placeholder="(선택) loci 목록 텍스트 파일" />
                <button onClick={() => void pickFile(setLociList, "loci 목록", ["txt", "tsv"])}>
                  찾아보기
                </button>
                {lociList && <button onClick={() => setLociList("")}>지우기</button>}
              </div>
            </div>
          </>
        )}

        <div className="field">
          <label htmlFor="output">결과 폴더</label>
          <div className="row">
            <input id="output" type="text" value={outputDir} readOnly placeholder="폴더를 선택하세요" />
            <button onClick={() => void pickDir(setOutputDir)}>찾아보기</button>
          </div>
          <div className="hint">
            {module === "CreateSchema"
              ? "CreateSchema 의 산출물인 스키마 자체는 앱 저장소에 보관됩니다. 이 폴더는 로그와 내보내기 기본 위치로 쓰입니다."
              : "AlleleCall 결과가 이 폴더로 회수됩니다."}
          </div>
        </div>

        <div className="field">
          <label htmlFor="cpu">CPU 개수 — 선택</label>
          <input
            id="cpu"
            type="number"
            min={1}
            value={cpu}
            onChange={(e) => setCpu(e.target.value)}
            placeholder={backend?.cpuCount ? `기본값: ${backend.cpuCount}` : "비우면 자동"}
          />
          <div className="hint">
            비워두면 WSL 내부에서 확인한 코어 수를 사용합니다. Windows 논리 코어 수와 다를 수
            있습니다.
          </div>
        </div>

        <div className="row">
          <button className="primary" disabled={!ready || busy} onClick={() => void submit()}>
            {busy ? "등록 중..." : "실행"}
          </button>
        </div>
      </div>
    </>
  );
}
