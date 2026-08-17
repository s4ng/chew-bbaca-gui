import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import TrainingFileField from "../components/TrainingFileField";
import { MODULE_STEP } from "../lib/format";
import { useT } from "../lib/i18n";
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

/**
 * 1단계는 CreateSchema 와 PrepExternalSchema 가 나눠 갖는다.
 * 4는 표준 3단계 바깥의 후처리·점검 도구들이 모이는 자리다 (format.ts 의 MODULE_STEP).
 */
const PIPELINE_STEPS = [1, 2, 3, 4];

/** 폼이 제공하는 모듈 — 이제 `Module` 전체와 같다. */
type FormModule = Module;

/**
 * `--cpu` 인자를 받지 않는 모듈. `runner/cli.rs` 의 `no_cpu` 와 같은 목록이어야 한다.
 * 어긋나면 한쪽은 칸을 보여주고 다른 쪽은 값을 버린다.
 */
const NO_CPU: Module[] = ["ExtractCgMLST", "RemoveGenes", "JoinProfiles"];

/**
 * 모듈 선택 목록의 순서. 표준 파이프라인 순서로 두고 후처리·점검을 뒤에 붙인다 —
 * 알파벳순이 아니라 **사용자가 실제로 밟는 순서**여야 목록이 안내 역할을 한다.
 */
const MODULE_ORDER: Module[] = [
  "CreateSchema",
  "AlleleCall",
  "ExtractCgMLST",
  "RemoveGenes",
  "JoinProfiles",
  "PrepExternalSchema",
  "SchemaEvaluator",
  "AlleleCallEvaluator",
];

/**
 * 고른 모듈이 무엇을 하는지, 그리고 파이프라인의 어디쯤인지 보여준다.
 *
 * 단계 번호를 붙이는 이유는 이 세 모듈이 **실제로 순서가 있는 절차**이기 때문이다.
 * 사용자가 가장 자주 막히는 곳은 개별 모듈의 사용법이 아니라 "이제 뭘 해야 하지" 다.
 */
function ModuleGuide({ module }: { module: Module }) {
  const t = useT();
  const info = t.moduleInfo[module];
  const step = MODULE_STEP[module];

  return (
    <div className="module-info">
      <div className="steps" aria-label={t.newJob.pipelineLabel}>
        {PIPELINE_STEPS.map((n) => (
          <span key={n} className={`step ${n === step ? "on" : ""}`}>
            {t.step[n]}
          </span>
        ))}
      </div>

      <p className="summary">{info.summary}</p>

      <table className="kv">
        <tbody>
          <tr>
            <td>{t.newJob.needs}</td>
            <td>{info.needs}</td>
          </tr>
          <tr>
            <td>{t.newJob.gives}</td>
            <td>{info.gives}</td>
          </tr>
        </tbody>
      </table>

      {info.next && (
        <p className="next">
          <strong>{t.newJob.nextStep(step)}</strong> {info.next}
        </p>
      )}
      {info.caution && <p className="caution">{info.caution}</p>}
    </div>
  );
}

export default function NewJobPage({ onSubmitted }: { onSubmitted: () => void }) {
  const t = useT();
  const [module, setModule] = useState<FormModule>("CreateSchema");
  const [genesList, setGenesList] = useState("");
  const [keepInstead, setKeepInstead] = useState(false);
  const [profilesFiles, setProfilesFiles] = useState<string[]>([]);
  const [commonOnly, setCommonOnly] = useState(false);
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
  const [resultsDir, setResultsDir] = useState("");
  const [lociReports, setLociReports] = useState(false);

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

  const pickProfiles = async () => {
    const picked = await open({
      multiple: true,
      filters: [{ name: "allelic profile", extensions: ["tsv"] }],
    });
    if (Array.isArray(picked)) setProfilesFiles(picked);
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
      // 모듈마다 필요한 것만 담는다. 타입이 조합을 강제하므로, 예전처럼
      // "이 모듈이면 이 값, 아니면 null" 을 줄줄이 쓸 필요가 없다.
      const common = { outputDir, cpu: cpu ? Number(cpu) : null };
      const spec: JobSpec =
        module === "CreateSchema"
          ? { ...common, module, inputDir, schemaName, ptf: ptf || null, cdsInput }
          : module === "AlleleCall"
            ? { ...common, module, inputDir, schemaId, lociList: lociList || null, cdsInput }
            : module === "PrepExternalSchema"
              ? { ...common, module, schemaDir: inputDir, schemaName, ptf: ptf || null }
              : module === "RemoveGenes"
                ? { ...common, module, profilesFile, genesList, keepInstead }
                : module === "JoinProfiles"
                  ? { ...common, module, profilesFiles, commonOnly }
                  : module === "SchemaEvaluator"
                    ? { ...common, module, schemaId, lociReports }
                    : module === "AlleleCallEvaluator"
                      ? { ...common, module, resultsDir, schemaId }
                      : { ...common, module, profilesFile, thresholds: thresholds.trim() || null };
      await jobsSubmit(spec);
      onSubmitted();
    } catch (e) {
      setError(asAppError(e).message);
    } finally {
      setBusy(false);
    }
  };

  // 스키마를 만드는 모듈은 산출물이 앱 저장소로 가므로 결과 폴더가 선택 항목이다.
  const producesSchema = module === "CreateSchema" || module === "PrepExternalSchema";
  const isEvaluator = module === "SchemaEvaluator" || module === "AlleleCallEvaluator";

  // 스키마 선택은 세 모듈이 그대로 공유한다 (AlleleCall · 평가 리포트 둘).
  const schemaField = (
    <div className="field">
      <label htmlFor="schema">{t.newJob.schema}</label>
      <select id="schema" value={schemaId} onChange={(e) => setSchemaId(e.target.value)}>
        <option value="">{t.common.select}</option>
        {schemas.map((s) => (
          <option key={s.schemaId} value={s.schemaId}>
            {s.name}
            {s.lociCount ? t.newJob.schemaLoci(s.lociCount) : ""}
          </option>
        ))}
      </select>
      {schemas.length === 0 && <div className="hint">{t.newJob.noSchema}</div>}
    </div>
  );
  const ready =
    (producesSchema || outputDir !== "") &&
    (producesSchema
      ? inputDir !== "" && schemaName.trim() !== ""
      : module === "AlleleCall"
        ? inputDir !== "" && schemaId !== ""
        : module === "RemoveGenes"
          ? profilesFile !== "" && profilesInfo?.looksValid === true && genesList !== ""
          : module === "JoinProfiles"
            ? profilesFiles.length >= 2
            : module === "SchemaEvaluator"
              ? schemaId !== ""
              : module === "AlleleCallEvaluator"
                ? resultsDir !== "" && schemaId !== ""
                : profilesFile !== "" && profilesInfo?.looksValid === true) &&
    // loci 목록은 선택이지만, 골랐다면 올바른 파일이어야 한다.
    (lociList === "" || lociInfo?.looksValid === true);

  return (
    <>
      <div className="page-head">
        <div>
          <h1>{t.newJob.title}</h1>
          <p>{t.newJob.subtitle}</p>
        </div>
      </div>

      {error && <div className="banner error">{error}</div>}
      {backend && !backend.ready && <div className="banner warn">{backend.detail}</div>}

      <div className="card">
        <div className="field">
          <label htmlFor="module">{t.newJob.moduleField}</label>
          <select
            id="module"
            value={module}
            onChange={(e) => setModule(e.target.value as FormModule)}
          >
            {/* 영문 모듈명은 chewBBACA 의 하위 명령 이름이라 번역하지 않는다. */}
            {MODULE_ORDER.map((m) => (
              <option key={m} value={m}>
                {m} — {t.moduleOption[m]}
              </option>
            ))}
          </select>
        </div>

        <ModuleGuide module={module} />
      </div>

      <div className="card">

        {module === "SchemaEvaluator" ? (
          // 입력이 앱 저장소의 스키마뿐이라 고를 경로가 없다.
          schemaField
        ) : module === "AlleleCallEvaluator" ? (
          <>
            <div className="field">
              <label htmlFor="results">{t.newJob.resultsDir}</label>
              <div className="row">
                <input
                  id="results"
                  type="text"
                  value={resultsDir}
                  readOnly
                  placeholder={t.newJob.resultsDirPlaceholder}
                />
                <button onClick={() => void pickDir(setResultsDir)}>{t.common.browse}</button>
              </div>
              <div className="hint">{t.newJob.resultsDirHint}</div>
            </div>
            {schemaField}
          </>
        ) : module === "PrepExternalSchema" ? (
          <div className="field">
            <label htmlFor="input">{t.newJob.externalSchemaDir}</label>
            <div className="row">
              <input
                id="input"
                type="text"
                value={inputDir}
                readOnly
                placeholder={t.newJob.externalSchemaPlaceholder}
              />
              <button onClick={() => void pickDir(setInputDir)}>{t.common.browse}</button>
            </div>
            <div className="hint">
              {inputInfo
                ? t.newJob.inputSummary(inputInfo.totalFiles, inputInfo.fastaFiles)
                : t.newJob.externalSchemaHint}
            </div>
          </div>
        ) : module === "JoinProfiles" ? (
          <div className="field">
            <label>{t.newJob.joinLabel}</label>
            <div className="row">
              <button onClick={() => void pickProfiles()}>{t.newJob.pickFiles}</button>
              {profilesFiles.length > 0 && (
                <button onClick={() => setProfilesFiles([])}>{t.newJob.clearFiles}</button>
              )}
            </div>
            {profilesFiles.length > 0 ? (
              <ul className="detail-list" style={{ marginTop: 8 }}>
                {profilesFiles.map((f) => (
                  <li key={f} className="path">{f}</li>
                ))}
              </ul>
            ) : (
              <div className="hint">{t.newJob.joinHint}</div>
            )}
          </div>
        ) : module === "ExtractCgMLST" || module === "RemoveGenes" ? (
          <div className="field">
            <label htmlFor="profiles">{t.newJob.profilesFile}</label>
            <div className="row">
              <input
                id="profiles"
                type="text"
                value={profilesFile}
                readOnly
                placeholder={t.newJob.profilesPlaceholder}
              />
              <button onClick={() => void pickFile(setProfilesFile, "allelic profile", ["tsv"])}>
                {t.common.browse}
              </button>
            </div>
            {profilesInfo && !profilesInfo.looksValid ? (
              <div className="banner error" style={{ marginTop: 8, marginBottom: 0 }}>
                {t.newJob.profilesInvalid(profilesInfo.firstColumn, profilesInfo.loci + 1)}
                <br />
                {t.newJob.profilesInvalidHelp}
              </div>
            ) : (
              <div className="hint">
                {profilesInfo
                  ? t.newJob.profilesSummary(profilesInfo.genomes, profilesInfo.loci)
                  : t.newJob.profilesHint}
              </div>
            )}
          </div>
        ) : (
          <div className="field">
            <label htmlFor="input">{t.newJob.assemblyDir}</label>
            <div className="row">
              <input
                id="input"
                type="text"
                value={inputDir}
                readOnly
                placeholder={t.newJob.assemblyPlaceholder}
              />
              <button onClick={() => void pickDir(setInputDir)}>{t.common.browse}</button>
            </div>
            <div className="hint">
              {inputInfo
                ? t.newJob.inputSummary(inputInfo.totalFiles, inputInfo.fastaFiles)
                : t.newJob.assemblyHint}
            </div>
          </div>
        )}

        {module === "CreateSchema" && (
          <>
            <div className="field">
              <label htmlFor="schemaName">{t.newJob.schemaName}</label>
              <input
                id="schemaName"
                type="text"
                value={schemaName}
                onChange={(e) => setSchemaName(e.target.value)}
                placeholder={t.newJob.schemaNamePlaceholder}
              />
              <div className="hint">{t.newJob.schemaNameHint}</div>
            </div>

            <TrainingFileField
              id="ptf"
              value={ptf}
              onChange={setPtf}
              hint={t.newJob.ptfHintCreate}
            />
          </>
        )}

        {module === "AlleleCall" && (
          <>
            {schemaField}

            <div className="field">
              <label htmlFor="gl">{t.newJob.lociListLabel}</label>
              <div className="row">
                <input
                  id="gl"
                  type="text"
                  value={lociList}
                  readOnly
                  placeholder={t.newJob.lociListPlaceholder}
                />
                <button
                  onClick={() =>
                    void pickFile(setLociList, t.newJob.lociListFilter, ["txt", "tsv"])
                  }
                >
                  {t.common.browse}
                </button>
                {lociList && <button onClick={() => setLociList("")}>{t.common.clear}</button>}
              </div>
              {lociInfo && !lociInfo.looksValid ? (
                <div className="banner error" style={{ marginTop: 8, marginBottom: 0 }}>
                  {t.newJob.lociListInvalid(lociInfo.tabbed)}
                  <br />
                  {t.newJob.lociListInvalidHelp}
                </div>
              ) : (
                <div className="hint">
                  {lociInfo
                    ? t.newJob.lociListSummary(lociInfo.loci)
                    : t.newJob.lociListHint}
                </div>
              )}
            </div>
          </>
        )}

        {module === "PrepExternalSchema" && (
          <>
            <div className="field">
              <label htmlFor="schemaName">{t.newJob.schemaName}</label>
              <input
                id="schemaName"
                type="text"
                value={schemaName}
                onChange={(e) => setSchemaName(e.target.value)}
                placeholder={t.newJob.externalNamePlaceholder}
              />
              <div className="hint">{t.newJob.externalNameHint}</div>
            </div>
            <TrainingFileField id="ptf2" value={ptf} onChange={setPtf} hint={t.newJob.ptfHintPrep} />
          </>
        )}

        {module === "RemoveGenes" && (
          <>
            <div className="field">
              <label htmlFor="genes">{t.newJob.genesList}</label>
              <div className="row">
                <input
                  id="genes"
                  type="text"
                  value={genesList}
                  readOnly
                  placeholder={t.newJob.genesListPlaceholder}
                />
                <button
                  onClick={() =>
                    void pickFile(setGenesList, t.newJob.lociListFilter, ["txt", "tsv"])
                  }
                >
                  {t.common.browse}
                </button>
                {genesList && <button onClick={() => setGenesList("")}>{t.common.clear}</button>}
              </div>
            </div>
            <div className="field">
              <label className="inline-check">
                <input
                  type="checkbox"
                  checked={keepInstead}
                  onChange={(e) => setKeepInstead(e.target.checked)}
                />
                {t.newJob.keepInstead}
              </label>
              <div className="hint">{t.newJob.keepInsteadHint}</div>
            </div>
          </>
        )}

        {module === "JoinProfiles" && (
          <div className="field">
            <label className="inline-check">
              <input
                type="checkbox"
                checked={commonOnly}
                onChange={(e) => setCommonOnly(e.target.checked)}
              />
              {t.newJob.commonOnly}
            </label>
            <div className="hint">{t.newJob.commonOnlyHint}</div>
          </div>
        )}

        {module === "SchemaEvaluator" && (
          <div className="field">
            <label className="inline-check">
              <input
                type="checkbox"
                checked={lociReports}
                onChange={(e) => setLociReports(e.target.checked)}
              />
              {t.newJob.lociReports}
            </label>
            <div className="hint">{t.newJob.lociReportsHint}</div>
          </div>
        )}

        {module === "ExtractCgMLST" && (
          <div className="field">
            <label htmlFor="thr">{t.newJob.thresholds}</label>
            <input
              id="thr"
              type="text"
              value={thresholds}
              onChange={(e) => setThresholds(e.target.value)}
              placeholder={t.newJob.thresholdsPlaceholder}
            />
            <div className="hint">{t.newJob.thresholdsHint}</div>
          </div>
        )}

        {/* 어셈블리를 입력으로 받는 두 모듈에만 있다. */}
        {(module === "CreateSchema" || module === "AlleleCall") && (
          <div className="field">
            <label className="inline-check">
              <input
                type="checkbox"
                checked={cdsInput}
                onChange={(e) => setCdsInput(e.target.checked)}
              />
              {t.newJob.cdsInput}
            </label>
            <div className="hint">{t.newJob.cdsInputHint}</div>
          </div>
        )}

        <div className="field">
          <label htmlFor="output">
            {t.newJob.outputDir}
            {producesSchema ? t.common.optional : ""}
          </label>
          <div className="row">
            <input
              id="output"
              type="text"
              value={outputDir}
              readOnly
              placeholder={
                producesSchema
                  ? t.newJob.outputOptionalPlaceholder
                  : t.newJob.assemblyPlaceholder
              }
            />
            <button onClick={() => void pickDir(setOutputDir)}>{t.common.browse}</button>
            {producesSchema && outputDir && (
              <button onClick={() => setOutputDir("")}>{t.common.clear}</button>
            )}
          </div>
          <div className="hint">
            {producesSchema
              ? t.newJob.outputHintSchema
              : module === "AlleleCall"
                ? t.newJob.outputHintAlleleCall
                : isEvaluator
                  ? t.newJob.outputHintEvaluator
                  : t.newJob.outputHintExtract}
          </div>
        </div>

        {/* 이 셋에는 --cpu 인자가 아예 없다 (cli.rs 의 `no_cpu`). 칸을 띄워두면
            값을 넣어도 아무 일이 없는데 사용자는 반영됐다고 믿는다. */}
        {!NO_CPU.includes(module) && (
        <div className="field">
          <label htmlFor="cpu">{t.newJob.cpu}</label>
          <input
            id="cpu"
            type="number"
            min={1}
            value={cpu}
            onChange={(e) => setCpu(e.target.value)}
            placeholder={
              backend?.cpuCount ? t.newJob.cpuDefault(backend.cpuCount) : t.newJob.cpuAuto
            }
          />
          <div className="hint">{t.newJob.cpuHint}</div>
        </div>
        )}

        <div className="row">
          <button className="primary" disabled={!ready || busy} onClick={() => void submit()}>
            {busy ? t.newJob.submitting : t.newJob.submit}
          </button>
        </div>
      </div>
    </>
  );
}
