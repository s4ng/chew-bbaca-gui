// Tauri IPC 의 유일한 통로.
//
// 컴포넌트가 `invoke("...")` 를 직접 부르지 않게 한다 — 명령 이름 오타는
// 런타임에야 드러나고, 그때는 이미 사용자가 40분짜리 작업을 시작한 뒤다.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  BackendStatus,
  DataDirInfo,
  DiskUsage,
  EnvReport,
  FirmwareHint,
  GenomeScan,
  InputDirInfo,
  Job,
  JobSpec,
  LociListInfo,
  LogEvent,
  McpStatus,
  ProgressEvent,
  ProfilesInfo,
  ProvisionEvent,
  PruneResult,
  RootfsOrigin,
  SchemaInfo,
  Settings,
  StateEvent,
  TrainingCreated,
  TrainingFile,
  WorkDirEntry,
} from "./types";

// ---------------------------------------------------------------- 환경

export const envProbe = () => invoke<EnvReport>("env_probe");
export const backendStatus = () => invoke<BackendStatus>("backend_status");
export const envInstallWsl = () => invoke<string>("env_install_wsl");
export const envManualCommands = () => invoke<string[]>("env_manual_commands");
export const envFirmwareHint = (manufacturer: string | null) =>
  invoke<FirmwareHint>("env_firmware_hint", { manufacturer });
export const envRebootToFirmware = () => invoke<void>("env_reboot_to_firmware");
export const envRootfsOrigin = () => invoke<RootfsOrigin>("env_rootfs_origin");
export const envProvision = () => invoke<void>("env_provision");
export const envUnregister = () => invoke<void>("env_unregister");

// ---------------------------------------------------------------- 디스크

export const diskCompact = () => invoke<string>("disk_compact");
export const diskUsage = () => invoke<DiskUsage>("disk_usage");
/** 데이터 폴더가 지금 어디이고 아직 옮길 수 있는지. */
export const dataDirInfo = () => invoke<DataDirInfo>("data_dir_info");
/**
 * 데이터 폴더 위치를 바꾼다. 반환값은 **실제로 기록된 경로**다 — 고른 폴더 아래에
 * `ChewieApp` 이 붙으므로 사용자가 고른 것과 다르다.
 *
 * 반영은 다음 기동부터다. 부른 쪽이 이어서 `appRestart()` 를 부른다.
 */
export const dataDirSet = (path: string) => invoke<string>("data_dir_set", { path });
/** 앱을 다시 시작한다. 돌아오지 않는다. */
export const appRestart = () => invoke<void>("app_restart");
export const workPrunable = () => invoke<WorkDirEntry[]>("work_prunable");
export const workPrune = (jobIds: string[]) => invoke<PruneResult>("work_prune", { jobIds });

// ---------------------------------------------------------------- 작업

export const jobsSubmit = (spec: JobSpec) => invoke<string>("jobs_submit", { spec });
export const jobsList = (limit = 100) => invoke<Job[]>("jobs_list", { limit });
export const jobsGet = (jobId: string) => invoke<Job | null>("jobs_get", { jobId });
export const jobsCancel = (jobId: string) => invoke<void>("jobs_cancel", { jobId });
export const jobsLog = (jobId: string) => invoke<string>("jobs_log", { jobId });
export const jobsReconcile = () => invoke<Job[]>("jobs_reconcile");
/** 이어받은 작업 중 아직 실행 중인 것. 화면을 다시 열 때마다 물어도 된다. */
export const jobsAdopted = () => invoke<Job[]>("jobs_adopted");
/** 평가 리포트 HTML 을 기본 브라우저로 연다. 반환값은 연 파일의 경로. */
export const reportOpen = (jobId: string) => invoke<string>("report_open", { jobId });

// ---------------------------------------------------------------- 스키마

export const schemasList = () => invoke<SchemaInfo[]>("schemas_list");
export const schemasDelete = (schemaId: string) => invoke<void>("schemas_delete", { schemaId });
export const schemasImport = (dir: string, name: string) =>
  invoke<SchemaInfo>("schemas_import", { dir, name });
export const schemasExport = (schemaId: string, dest: string) =>
  invoke<string>("schemas_export", { schemaId, dest });

// ---------------------------------------------------------------- training file

export const trainingList = () => invoke<TrainingFile[]>("training_list");
/**
 * 게놈 폴더를 훑어 학습 후보를 추린다. 파일을 만들지 않는다.
 *
 * **폴더의 FASTA 를 전부 읽는다** — contig 수는 파일 크기로 알 수 없다.
 * 게놈 수백 개면 수 초가 걸리므로 부르는 쪽에서 진행 표시를 띄운다.
 */
export const trainingScan = (path: string) => invoke<GenomeScan>("training_scan", { path });
/** 게놈 하나를 골라 학습시키고 저장소에 넣는다. 수십 초 걸린다. */
export const trainingCreate = (name: string, genomeDir: string, genomeFile?: string | null) =>
  invoke<TrainingCreated>("training_create", { name, genomeDir, genomeFile: genomeFile ?? null });
/** 되돌릴 수 없다. 확인을 받은 뒤 부른다. */
export const trainingDelete = (name: string) => invoke<void>("training_delete", { name });

// ---------------------------------------------------------------- 설정

export const settingsGet = () => invoke<Settings>("settings_get");
export const settingsSet = (settings: Settings) => invoke<void>("settings_set", { settings });
export const inspectInputDir = (path: string) => invoke<InputDirInfo>("inspect_input_dir", { path });
export const inspectProfilesFile = (path: string) =>
  invoke<ProfilesInfo>("inspect_profiles_file", { path });
export const inspectLociList = (path: string) =>
  invoke<LociListInfo>("inspect_loci_list", { path });
/** 따라해보기 가이드를 앱 폴더에 꺼내 기본 브라우저로 연다. 반환값은 그 파일 경로. */
export const guideOpen = () => invoke<string>("guide_open");

// ---------------------------------------------------------------- MCP

export const mcpStatus = () => invoke<McpStatus>("mcp_status");
/** MCP 연결 안내 문서를 앱 폴더에 꺼내 기본 브라우저로 연다. */
export const mcpGuideOpen = () => invoke<string>("mcp_guide_open");
/**
 * MCP 설정을 바꾸고 서버를 다시 띄운다.
 *
 * **포트·켬/끔은 `settingsSet` 으로 바꾸면 안 된다** — 그쪽은 값만 저장하고
 * 실제 리스너는 그대로다.
 */
export const mcpConfigure = (enabled: boolean, port: number, allowRun: boolean) =>
  invoke<McpStatus>("mcp_configure", { enabled, port, allowRun });
/** 토큰을 새로 발급한다. 기존 클라이언트 설정은 즉시 무효가 된다. */
export const mcpRegenerateToken = () => invoke<McpStatus>("mcp_regenerate_token");

// ---------------------------------------------------------------- 이벤트

export const onLog = (fn: (e: LogEvent) => void): Promise<UnlistenFn> =>
  listen<LogEvent>("job://log", (e) => fn(e.payload));

export const onState = (fn: (e: StateEvent) => void): Promise<UnlistenFn> =>
  listen<StateEvent>("job://state", (e) => fn(e.payload));

export const onProgress = (fn: (e: ProgressEvent) => void): Promise<UnlistenFn> =>
  listen<ProgressEvent>("job://progress", (e) => fn(e.payload));

export const onProvision = (fn: (e: ProvisionEvent) => void): Promise<UnlistenFn> =>
  listen<ProvisionEvent>("env://provision", (e) => fn(e.payload));
