import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { MODULE_INFO, MODULE_LABEL, MODULE_STEP } from "../lib/format";
import {
  backendStatus,
  inspectInputDir,
  inspectLociList,
  inspectProfilesFile,
  jobsSubmit,
  schemasList,
  settingsGet,
} from "../lib/ipc";
import {
  asAppError,
  type BackendStatus,
  type InputDirInfo,
  type JobSpec,
  type LociListInfo,
  type Module,
  type ProfilesInfo,
  type SchemaInfo,
} from "../lib/types";

const PIPELINE: Module[] = ["CreateSchema", "AlleleCall", "ExtractCgMLST"];

/**
 * 고른 모듈이 무엇을 하는지, 그리고 파이프라인의 어디쯤인지 보여준다.
 *
 * 단계 번호를 붙이는 이유는 이 세 모듈이 **실제로 순서가 있는 절차**이기 때문이다.
 * 사용자가 가장 자주 막히는 곳은 개별 모듈의 사용법이 아니라 "이제 뭘 해야 하지" 다.
 */
function ModuleGuide({ module }: { module: Module }) {
  const info = MODULE_INFO[module];
  const step = MODULE_STEP[module];

  return (
    <div className="module-info">
      <div className="steps" aria-label="일반적인 실행 순서">
        {PIPELINE.map((m) => (
          <span key={m} className={`step ${m === module ? "on" : ""}`}>
            {MODULE_STEP[m]}. {MODULE_LABEL[m]}
          </span>
        ))}
      </div>

      <p className="summary">{info.summary}</p>

      <table className="kv">
        <tbody>
          <tr>
            <td>필요한 것</td>
            <td>{info.needs}</td>
          </tr>
          <tr>
            <td>나오는 것</td>
            <td>{info.gives}</td>
          </tr>
        </tbody>
      </table>

      {info.next && (
        <p className="next">
          <strong>다음 단계 ({step === 3 ? "2로 되돌아감" : `${step + 1}단계`})</strong> {info.next}
        </p>
      )}
      {info.caution && <p className="caution">{info.caution}</p>}
    </div>
  );
}

export default function NewJobPage({ onSubmitted }: { onSubmitted: () => void }) {
  const [module, setModule] = useState<Module>("CreateSchema");
  const [inputDir, setInputDir] = useState("");
  const [outputDir, setOutputDir] = useState("");
  const [schemaName, setSchemaName] = useState("");
  const [schemaId, setSchemaId] = useState("");
  const [ptf, setPtf] = useState("");
  const [lociList, setLociList] = useState("");
  const [lociInfo, setLociInfo] = useState<LociListInfo | null>(null);
  const [cdsInput, setCdsInput] = useState(false);
  const [cpu, setCpu] = useState("");
  const [profilesFile, setProfilesFile] = useState("");
  const [profilesInfo, setProfilesInfo] = useState<ProfilesInfo | null>(null);
  const [thresholds, setThresholds] = useState("");

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

  // 고른 즉시 진짜 allelic profile 표인지 확인한다. AlleleCall 결과 폴더에는 TSV 가
  // 일곱 개 있고, 엉뚱한 것을 넣으면 chewBBACA 가 거절하지 않고 한참 헛돈다.
  useEffect(() => {
    if (!profilesFile) {
      setProfilesInfo(null);
      return;
    }
    let cancelled = false;
    inspectProfilesFile(profilesFile)
      .then((info) => !cancelled && setProfilesInfo(info))
      .catch((e) => {
        if (!cancelled) {
          setProfilesInfo(null);
          setError(asAppError(e).message);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [profilesFile]);

  // loci 목록도 고른 즉시 확인한다. 프로파일 표를 여기에 넣는 것이 흔한 실수다.
  useEffect(() => {
    if (!lociList) {
      setLociInfo(null);
      return;
    }
    let cancelled = false;
    inspectLociList(lociList)
      .then((info) => !cancelled && setLociInfo(info))
      .catch(() => !cancelled && setLociInfo(null));
    return () => {
      cancelled = true;
    };
  }, [lociList]);

  const submit = async () => {
    setError(null);
    setBusy(true);
    try {
      const spec: JobSpec = {
        module,
        inputDir: module === "ExtractCgMLST" ? "" : inputDir,
        outputDir,
        // ExtractCgMLST 에는 --cds 개념이 없다.
        cdsInput: module === "ExtractCgMLST" ? false : cdsInput,
        schemaId: module === "AlleleCall" ? schemaId : null,
        schemaName: module === "CreateSchema" ? schemaName : null,
        ptf: module === "CreateSchema" && ptf ? ptf : null,
        lociList: module === "AlleleCall" && lociList ? lociList : null,
        cpu: cpu ? Number(cpu) : null,
        profilesFile: module === "ExtractCgMLST" ? profilesFile : null,
        thresholds: module === "ExtractCgMLST" && thresholds.trim() ? thresholds.trim() : null,
      };
      await jobsSubmit(spec);
      onSubmitted();
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(false);
    }
  };

  // CreateSchema 는 산출물이 앱 저장소로 가므로 결과 폴더가 선택 항목이다.
  const ready =
    (module === "CreateSchema" || outputDir !== "") &&
    (module === "CreateSchema"
      ? inputDir !== "" && schemaName.trim() !== ""
      : module === "AlleleCall"
        ? inputDir !== "" && schemaId !== ""
        : profilesFile !== "" && profilesInfo?.looksValid === true) &&
    // loci 목록은 선택이지만, 골랐다면 올바른 파일이어야 한다.
    (lociList === "" || lociInfo?.looksValid === true);

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
            <option value="ExtractCgMLST">ExtractCgMLST — allele 결과에서 core genome 추출</option>
          </select>
        </div>

        <ModuleGuide module={module} />
      </div>

      <div className="card">

        {module === "ExtractCgMLST" ? (
          <div className="field">
            <label htmlFor="profiles">AlleleCall 결과 파일</label>
            <div className="row">
              <input
                id="profiles"
                type="text"
                value={profilesFile}
                readOnly
                placeholder="results_alleles.tsv 를 선택하세요"
              />
              <button
                onClick={() => void pickFile(setProfilesFile, "allelic profile", ["tsv"])}
              >
                찾아보기
              </button>
            </div>
            {profilesInfo && !profilesInfo.looksValid ? (
              <div className="banner error" style={{ marginTop: 8, marginBottom: 0 }}>
                이 파일은 allelic profile 표가 아닙니다 — 첫 열이{" "}
                <code>{profilesInfo.firstColumn}</code>, 열 {profilesInfo.loci + 1}개입니다.
                <br />
                AlleleCall 결과 폴더의 <code>results_alleles.tsv</code> 를 선택하세요. 같은
                폴더의 다른 TSV(<code>cds_coordinates.tsv</code> 등)를 넣으면 각 행이 균주로
                취급되어 오래 실행된 뒤 쓸모없는 결과가 나옵니다.
              </div>
            ) : (
              <div className="hint">
                {profilesInfo
                  ? `균주 ${profilesInfo.genomes}개 × loci ${profilesInfo.loci}개`
                  : "AlleleCall 결과 폴더 안의 results_alleles.tsv 입니다. 이 모듈은 어셈블리를 다시 읽지 않고 그 표만 봅니다."}
              </div>
            )}
          </div>
        ) : (
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
        )}

        {module === "CreateSchema" && (
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

          </>
        )}

        {module === "AlleleCall" && (
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
              {lociInfo && !lociInfo.looksValid ? (
                <div className="banner error" style={{ marginTop: 8, marginBottom: 0 }}>
                  loci 목록 파일이 아닙니다
                  {lociInfo.tabbed ? " — 탭으로 나뉜 표입니다." : " — 비어 있습니다."}
                  <br />
                  ExtractCgMLST 가 만든 <code>cgMLSTschema95.txt</code> 처럼 한 줄에 loci
                  이름 하나만 있는 파일을 선택하세요.
                </div>
              ) : (
                <div className="hint">
                  {lociInfo
                    ? `loci ${lociInfo.loci}개를 대상으로 실행합니다`
                    : "이 목록은 ExtractCgMLST 가 만들어 줍니다 (cgMLSTschema95.txt 등). 비워두면 스키마의 모든 loci 를 대상으로 합니다."}
                </div>
              )}
            </div>
          </>
        )}

        {module === "ExtractCgMLST" && (
          <div className="field">
            <label htmlFor="thr">존재 임계값 (--t) — 선택</label>
            <input
              id="thr"
              type="text"
              value={thresholds}
              onChange={(e) => setThresholds(e.target.value)}
              placeholder="비우면 0.95 / 0.99 / 1 을 모두 계산"
            />
            <div className="hint">
              어떤 loci 를 core 로 볼지 정하는 기준입니다. 0.95 면 &quot;균주의 95% 이상에
              존재하는 loci&quot; 를 뜻합니다. 공백으로 구분해 여러 값을 넣을 수 있고, 값마다
              결과 한 벌씩 나옵니다.
            </div>
          </div>
        )}

        {/* 어셈블리를 입력으로 받는 두 모듈에 공통이다. ExtractCgMLST 는 해당 없음. */}
        {module !== "ExtractCgMLST" && (
          <div className="field">
            <label className="inline-check">
              <input
                type="checkbox"
                checked={cdsInput}
                onChange={(e) => setCdsInput(e.target.checked)}
              />
              입력이 이미 CDS 입니다 (--cds)
            </label>
            <div className="hint">
              게놈 전체가 아니라 단백질 코딩 서열만 담긴 FASTA 라면 켜세요. 유전자 예측
              (Prodigal)을 건너뜁니다. 잘못 켜면 결과가 크게 달라집니다.
            </div>
          </div>
        )}

        <div className="field">
          <label htmlFor="output">
            결과 폴더{module === "CreateSchema" ? " — 선택" : ""}
          </label>
          <div className="row">
            <input
              id="output"
              type="text"
              value={outputDir}
              readOnly
              placeholder={
                module === "CreateSchema" ? "(선택) 비워두어도 됩니다" : "폴더를 선택하세요"
              }
            />
            <button onClick={() => void pickDir(setOutputDir)}>찾아보기</button>
            {module === "CreateSchema" && outputDir && (
              <button onClick={() => setOutputDir("")}>지우기</button>
            )}
          </div>
          <div className="hint">
            {module === "CreateSchema"
              ? "만들어진 스키마는 앱 저장소에 보관되고 [스키마] 화면에서 관리합니다. 이 폴더를 지정하면 실행 로그 사본만 남습니다 — 스키마 파일은 [스키마] → [내보내기] 로 꺼냅니다."
              : module === "AlleleCall"
                ? "AlleleCall 결과가 이 폴더로 회수됩니다."
                : "cgMLST 프로파일과 loci 목록(cgMLSTschema*.txt)이 이 폴더로 회수됩니다."}
          </div>
        </div>

        {/* ExtractCgMLST 에는 --cpu 인자가 없다 (cli.rs 참조). */}
        {module !== "ExtractCgMLST" && (
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
        )}

        <div className="row">
          <button className="primary" disabled={!ready || busy} onClick={() => void submit()}>
            {busy ? "등록 중..." : "실행"}
          </button>
        </div>
      </div>
    </>
  );
}
