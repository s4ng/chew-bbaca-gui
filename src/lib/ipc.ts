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
  LogEvent,
  ProgressEvent,
  ProvisionEvent,
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

// ---------------------------------------------------------------- 스키마

export const schemasList = () => invoke<SchemaInfo[]>("schemas_list");
export const schemasDelete = (schemaId: string) => invoke<void>("schemas_delete", { schemaId });
export const schemasExport = (schemaId: string, dest: string) =>
  invoke<string>("schemas_export", { schemaId, dest });

// ---------------------------------------------------------------- 설정

export const settingsGet = () => invoke<Settings>("settings_get");
export const settingsSet = (settings: Settings) => invoke<void>("settings_set", { settings });
export const inspectInputDir = (path: string) => invoke<InputDirInfo>("inspect_input_dir", { path });

// ---------------------------------------------------------------- 이벤트

export const onLog = (fn: (e: LogEvent) => void): Promise<UnlistenFn> =>
  listen<LogEvent>("job://log", (e) => fn(e.payload));

export const onState = (fn: (e: StateEvent) => void): Promise<UnlistenFn> =>
  listen<StateEvent>("job://state", (e) => fn(e.payload));

export const onProgress = (fn: (e: ProgressEvent) => void): Promise<UnlistenFn> =>
  listen<ProgressEvent>("job://progress", (e) => fn(e.payload));

export const onProvision = (fn: (e: ProvisionEvent) => void): Promise<UnlistenFn> =>
  listen<ProvisionEvent>("env://provision", (e) => fn(e.payload));
