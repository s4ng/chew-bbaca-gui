// Rust 쪽 serde 표현과 1:1 로 대응한다.
// 필드 이름이 어긋나면 조용히 undefined 가 되므로, Rust 구조체를 고칠 때
// 반드시 이 파일도 함께 고친다.

export type Module = "CreateSchema" | "AlleleCall" | "ExtractCgMLST";

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

export interface JobSpec {
  module: Module;
  inputDir: string;
  outputDir: string;
  schemaId?: string | null;
  schemaName?: string | null;
  ptf?: string | null;
  cdsInput: boolean;
  lociList?: string | null;
  cpu?: number | null;
  /** ExtractCgMLST 입력: AlleleCall 이 만든 results_alleles.tsv */
  profilesFile?: string | null;
  /** ExtractCgMLST 의 --t. 비우면 기본값(0.95 / 0.99 / 1)을 모두 계산한다 */
  thresholds?: string | null;
}

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
