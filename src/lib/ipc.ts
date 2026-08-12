// Tauri IPC 의 유일한 통로.
//
// 컴포넌트가 `invoke("...")` 를 직접 부르지 않게 한다 — 명령 이름 오타는
// 런타임에야 드러나고, 그때는 이미 사용자가 40분짜리 작업을 시작한 뒤다.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  BackendStatus,
  DiskUsage,
  EnvReport,
  FirmwareHint,
  InputDirInfo,
  Job,
  JobSpec,
  LociListInfo,
  LogEvent,
  McpStatus,
  ProgressEvent,
  ProfilesInfo,
  ProvisionEvent,
  RootfsOrigin,
  SchemaInfo,
  Settings,
  StateEvent,
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
