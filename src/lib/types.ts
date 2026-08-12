// Rust 쪽 serde 표현과 1:1 로 대응한다.
// 필드 이름이 어긋나면 조용히 undefined 가 되므로, Rust 구조체를 고칠 때
// 반드시 이 파일도 함께 고친다.

/** Rust `Module` 과 1:1. 여덟 개 모두 폼에 노출된다. */
export type Module =
  | "CreateSchema"
  | "AlleleCall"
  | "ExtractCgMLST"
  | "PrepExternalSchema"
  | "RemoveGenes"
  | "JoinProfiles"
  | "SchemaEvaluator"
  | "AlleleCallEvaluator";

export type JobStatus = "queued" | "running" | "completed" | "failed" | "cancelled";

export interface Job {
  jobId: string;
  module: Module;
  status: JobStatus;
  args: string;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
  pgid: number | null;
  workDir: string | null;
  logPath: string | null;
  outputPath: string | null;
  exitCode: number | null;
  error: string | null;
  progress: number | null;
}

/**
 * 모든 모듈에 공통인 것. Rust `JobSpec` 과 1:1.
 *
 * 모듈별 필드는 아래 유니온이 들고 있다. 한 덩어리로 두면 어떤 조합이 유효한지가
 * 타입에 적히지 않아, 폼이 엉뚱한 조합을 보내도 컴파일이 통과한다.
 */
interface JobSpecCommon {
  /** 결과를 회수할 폴더. CreateSchema 는 비어 있어도 된다. */
  outputDir: string;
  cpu?: number | null;
}

export type JobSpec =
  | (JobSpecCommon & {
      module: "CreateSchema";
      inputDir: string;
      schemaName: string;
      ptf?: string | null;
      cdsInput: boolean;
    })
  | (JobSpecCommon & {
      module: "AlleleCall";
      inputDir: string;
      schemaId: string;
      lociList?: string | null;
      cdsInput: boolean;
    })
  | (JobSpecCommon & {
      module: "ExtractCgMLST";
      /** AlleleCall 이 만든 results_alleles.tsv */
      profilesFile: string;
      /** --t. 비우면 기본값(0.95 / 0.99 / 1)을 모두 계산한다 */
      thresholds?: string | null;
    })
  | (JobSpecCommon & {
      module: "PrepExternalSchema";
      /** 들여올 외부 스키마 폴더 (loci 마다 FASTA 하나) */
      schemaDir: string;
      schemaName: string;
      ptf?: string | null;
    })
  | (JobSpecCommon & {
      module: "RemoveGenes";
      profilesFile: string;
      /** 제거할 loci 목록 */
      genesList: string;
      /** 켜면 목록에 있는 것만 남긴다 (--inverse) */
      keepInstead: boolean;
    })
  | (JobSpecCommon & {
      module: "JoinProfiles";
      /** 합칠 표들. 두 개 이상 */
      profilesFiles: string[];
      /** 공통 loci 만으로 합칠지 (--common) */
      commonOnly: boolean;
    })
  | (JobSpecCommon & {
      module: "SchemaEvaluator";
      schemaId: string;
      /** loci 마다 상세 페이지를 만든다 (--loci-reports). 느려진다 */
      lociReports: boolean;
    })
  | (JobSpecCommon & {
      module: "AlleleCallEvaluator";
      /** AlleleCall 결과 **폴더** — 파일 하나가 아니다 */
      resultsDir: string;
      schemaId: string;
    });

export interface SchemaInfo {
  schemaId: string;
  name: string;
  createdAt: string;
  createdByJob: string | null;
  backendPath: string;
  ptf: string | null;
  lociCount: number | null;
}

/** §7.3 의 게이트. 다음에 보여줄 화면을 그대로 결정한다. */
export type Gate =
  | "ready"
  | "bios-virtualization"
  | "wsl-missing"
  | "distro-missing"
  | "unknown";

export interface EnvReport {
  gate: Gate;
  distro: string;
  distroReady: boolean;
  hypervisorPresent: boolean | null;
  virtualizationFirmwareEnabled: boolean | null;
  wslInstalled: boolean;
  wslStatusText: string | null;
  existingDistros: string[];
  manufacturer: string | null;
  model: string | null;
  messages: string[];
}

export interface BackendStatus {
  ready: boolean;
  distro: string;
  chewbbacaVersion: string | null;
  cpuCount: number | null;
  detail: string;
}

export interface FirmwareHint {
  entryKey: string;
  menuPath: string;
  manufacturer: string | null;
}

export interface RootfsSource {
  /** 비어 있는 것이 정상 — 그때는 인스톨러 동봉본을 쓴다. 덮어쓰기 수단이다. */
  url: string;
  sha256: string;
  fileName: string;
  version: string;
}

/** Rust `env::RootfsOrigin` 과 1:1. 온보딩 ③ 의 문구가 이 값으로 갈린다. */
export type RootfsOrigin = "bundled" | "localFile" | "remote" | "missing";

export interface Settings {
  distro: string;
  rootfs: RootfsSource;
  keepWorkDir: boolean;
  defaultCpu: number | null;
  lastOutputDir: string | null;
  mcp: McpSettings;
}

/** Rust `settings::McpSettings` 와 1:1 (`doc/MCP.md`). */
export interface McpSettings {
  enabled: boolean;
  port: number;
  /** 첫 기동에서 발급된다. 기본값은 빈 문자열이다. */
  token: string;
  /** 끄면 읽기 전용 — 실행 도구가 목록에서 사라진다. */
  allowRun: boolean;
}

/** Rust `mcp::McpStatus` 와 1:1. 포트는 충돌 시 설정값과 달라질 수 있다. */
export interface McpStatus {
  running: boolean;
  enabled: boolean;
  allowRun: boolean;
  port: number | null;
  url: string | null;
  token: string;
  /** 사용자가 클라이언트 설정 파일에 붙여넣을 조각 */
  clientConfig: string;
}

export interface DiskUsage {
  vhdxBytes: number | null;
  appDir: string;
}

export interface InputDirInfo {
  path: string;
  totalFiles: number;
  fastaFiles: number;
}

/** ExtractCgMLST 입력 파일 진단 (Rust `commands::ProfilesInfo` 와 1:1) */
export interface ProfilesInfo {
  genomes: number;
  loci: number;
  firstColumn: string;
  looksValid: boolean;
}

/** `--gl` loci 목록 파일 진단 (Rust `commands::LociListInfo` 와 1:1) */
export interface LociListInfo {
  looksValid: boolean;
  loci: number;
  tabbed: boolean;
  firstLine: string;
}

// ---------------------------------------------------------------- 이벤트

export type LogStream = "stdout" | "stderr" | "app";

export interface LogEvent {
  jobId: string;
  stream: LogStream;
  line: string;
}

export interface StateEvent {
  jobId: string;
  status: JobStatus;
  message: string | null;
}

export interface ProgressEvent {
  jobId: string;
  fraction: number;
  label: string;
}

export interface ProvisionEvent {
  stage: "download" | "verify" | "import" | "done";
  message: string;
  fraction: number | null;
  ok: boolean | null;
}

/** Rust `Error` 의 직렬화 형태. `kind` 로 분기한다. */
export interface AppError {
  kind: string;
  message: string;
}

export function asAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "message" in e && "kind" in e) {
    return e as AppError;
  }
  return { kind: "unknown", message: String(e) };
}
