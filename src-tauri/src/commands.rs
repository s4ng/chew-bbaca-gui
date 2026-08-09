//! Tauri IPC 표면 (§2 의 `Presentation ↔ Application Core` 경계).
//!
//! 규칙 하나만 지키면 된다 — **여기서 WSL 이라는 단어가 프런트로 넘어가지
//! 않게 한다.** `env_*` 계열은 예외다. 온보딩은 본질적으로 Windows/WSL 절차라
//! UI 가 그 사실을 알아야 하며, macOS 확장 시 통째로 교체될 화면들이다(§9).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::db::Db;
use crate::env::probe::firmware_hint;
use crate::env::{probe, DownloadProgress, EnvReport, Provisioner};
use crate::error::{Error, Result};
use crate::jobs::JobManager;
use crate::models::{Job, JobSpec, SchemaInfo};
use crate::paths::AppPaths;
use crate::runner::BackendStatus;
use crate::schema_store::SchemaStore;
use crate::settings::Settings;

/// 온보딩 ③ 단계 진행 상황. 다운로드 진행률과 단계 전환을 함께 싣는다.
pub const EVENT_PROVISION: &str = "env://provision";

pub struct AppState {
    pub db: Arc<Db>,
    pub paths: AppPaths,
    pub manager: Arc<JobManager>,
}

impl AppState {
    fn settings(&self) -> Settings {
        Settings::load(&self.db)
    }

    fn provisioner(&self) -> Provisioner {
        Provisioner::new(self.paths.clone(), self.settings().distro)
    }

    fn schemas(&self) -> SchemaStore {
        SchemaStore::new(Arc::clone(&self.db), Arc::clone(self.manager.runner()))
    }
}

// Tauri 의 `emit` 은 페이로드를 창마다 복제하므로 Clone 이 필요하다.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionEvent {
    pub stage: &'static str,
    pub message: String,
    /// 다운로드 단계에서만 채워진다 (0.0 ~ 1.0). 총 크기를 모르면 `None`.
    pub fraction: Option<f32>,
    /// 전체 절차의 성공/실패. 진행 중에는 `None`.
    pub ok: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareHint {
    pub entry_key: String,
    pub menu_path: String,
    pub manufacturer: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskUsage {
    pub vhdx_bytes: Option<u64>,
    pub app_dir: String,
}

// ================================================================ 환경

/// §7.3 의 게이트 판정. 부작용 없이 읽기만 한다.
#[tauri::command]
pub fn env_probe(state: State<'_, AppState>) -> Result<EnvReport> {
    probe(&state.settings().distro)
}

/// 백엔드(배포판 + chewBBACA) 상태. 게이트 통과 후 상세 확인용.
#[tauri::command]
pub fn backend_status(state: State<'_, AppState>) -> BackendStatus {
    state.manager.runner().status()
}

/// 권한 상승 헬퍼로 `wsl --install --no-distribution` 을 대행한다 (§7.5).
///
/// UAC 가 거부되면 `elevation-denied` 로 돌아가므로, UI 는 그때
/// `env_manual_commands()` 안내로 폴백해야 한다.
#[tauri::command]
pub fn env_install_wsl(state: State<'_, AppState>) -> Result<String> {
    let p = state.provisioner();
    let message = p.install_wsl()?;
    // WSL1 이 기본값인 기기를 함께 정정한다 (§7.5-4).
    let _ = p.set_default_version_2();
    Ok(message)
}

/// 권한 상승이 거부됐을 때 보여줄 수동 명령. 버튼을 주되 수동 경로를 없애지 않는다.
#[tauri::command]
pub fn env_manual_commands() -> Vec<String> {
    Provisioner::manual_commands()
        .into_iter()
        .map(String::from)
        .collect()
}

/// 제조사별 BIOS 진입 안내 (§7.6-2).
#[tauri::command]
pub fn env_firmware_hint(manufacturer: Option<String>) -> FirmwareHint {
    let (entry_key, menu_path) = firmware_hint(manufacturer.as_deref());
    FirmwareHint {
        entry_key: entry_key.to_string(),
        menu_path: menu_path.to_string(),
        manufacturer,
    }
}

/// UEFI 로 즉시 재부팅한다. **재부팅을 유발하므로 UI 에서 확인을 받은 뒤 호출한다.**
#[tauri::command]
pub fn env_reboot_to_firmware(state: State<'_, AppState>) -> Result<()> {
    state.provisioner().reboot_to_firmware()
}

/// 배포판 게이트 ③ — rootfs 다운로드 → SHA256 검증 → `wsl --import`.
///
/// 수백 MB 를 받는 동안 UI 를 막지 않도록 별도 스레드에서 돌리고 진행 상황은
/// `env://provision` 이벤트로 보낸다.
#[tauri::command]
pub fn env_provision(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let settings = state.settings();
    if !settings.rootfs_ready() {
        return Err(Error::Other(
            "rootfs 배포 정보가 아직 설정되지 않았습니다. 설정 화면에서 URL 과 SHA256 을 입력하거나 릴리스를 기다려 주세요.".into(),
        ));
    }
    let provisioner = state.provisioner();
    let source = settings.rootfs.clone();

    std::thread::spawn(move || {
        let emit = |stage: &'static str, message: String, fraction: Option<f32>, ok: Option<bool>| {
            let _ = app.emit(
                EVENT_PROVISION,
                ProvisionEvent {
                    stage,
                    message,
                    fraction,
                    ok,
                },
            );
        };

        emit("download", "rootfs 다운로드를 시작합니다".into(), Some(0.0), None);
        let on_progress = |p: DownloadProgress| {
            let fraction = p.total.map(|t| p.received as f32 / t.max(1) as f32);
            let mb = p.received / (1024 * 1024);
            let total_mb = p.total.map(|t| t / (1024 * 1024));
            let message = match total_mb {
                Some(t) => format!("{mb} MB / {t} MB"),
                None => format!("{mb} MB"),
            };
            let _ = app.emit(
                EVENT_PROVISION,
                ProvisionEvent {
                    stage: "download",
                    message,
                    fraction,
                    ok: None,
                },
            );
        };

        let tarball = match provisioner.download_rootfs(&source, &on_progress) {
            Ok(p) => p,
            Err(e) => {
                emit("download", e.to_string(), None, Some(false));
                return;
            }
        };

        emit("verify", "체크섬 검증 완료".into(), None, None);
        emit("import", "배포판을 등록하는 중...".into(), None, None);

        match provisioner.import_distro(&tarball) {
            Ok(()) => emit("done", "환경 구성이 완료되었습니다".into(), None, Some(true)),
            Err(e) => emit("import", e.to_string(), None, Some(false)),
        }
    });

    Ok(())
}

/// 배포판 제거. 언인스톨이 한 줄로 끝나는 것이 전용 배포판을 쓰는 가장 큰 이유다.
#[tauri::command]
pub fn env_unregister(state: State<'_, AppState>) -> Result<()> {
    state.provisioner().unregister()
}

// ================================================================ 디스크

/// `ext4.vhdx` 는 파일을 지워도 자동으로 줄지 않는다 (§6.5).
#[tauri::command]
pub fn disk_compact(state: State<'_, AppState>) -> Result<String> {
    state.provisioner().compact_disk()
}

#[tauri::command]
pub fn disk_usage(state: State<'_, AppState>) -> DiskUsage {
    DiskUsage {
        vhdx_bytes: state.provisioner().vhdx_size(),
        app_dir: state.paths.root.to_string_lossy().to_string(),
    }
}

// ================================================================ 작업

#[tauri::command]
pub fn jobs_submit(state: State<'_, AppState>, spec: JobSpec) -> Result<String> {
    // 다음 실행의 기본값으로 기억해 둔다.
    let mut settings = state.settings();
    settings.last_output_dir = Some(spec.output_dir.clone());
    let _ = settings.save(&state.db);

    state.manager.submit(spec)
}

#[tauri::command]
pub fn jobs_list(state: State<'_, AppState>, limit: Option<i64>) -> Result<Vec<Job>> {
    state.manager.list(limit.unwrap_or(100))
}

#[tauri::command]
pub fn jobs_get(state: State<'_, AppState>, job_id: String) -> Result<Option<Job>> {
    state.manager.get(&job_id)
}

#[tauri::command]
pub fn jobs_cancel(state: State<'_, AppState>, job_id: String) -> Result<()> {
    state.manager.cancel(&job_id)
}

/// 로그 파일 전체. UI 가 이벤트를 놓쳤거나 앱을 다시 켠 경우 이걸로 복원한다.
#[tauri::command]
pub fn jobs_log(state: State<'_, AppState>, job_id: String) -> Result<String> {
    state.manager.read_log(&job_id)
}

/// 앱 시작 시 조정 (§6.3). 살아 있는 작업 목록을 돌려주면 UI 가
/// "이전 작업이 실행 중입니다 — 복구 / 종료" 를 띄운다.
#[tauri::command]
pub fn jobs_reconcile(state: State<'_, AppState>) -> Result<Vec<Job>> {
    state.manager.reconcile()
}

// ================================================================ 스키마

#[tauri::command]
pub fn schemas_list(state: State<'_, AppState>) -> Result<Vec<SchemaInfo>> {
    state.schemas().list()
}

#[tauri::command]
pub fn schemas_delete(state: State<'_, AppState>, schema_id: String) -> Result<()> {
    state.schemas().delete(&schema_id)
}

#[tauri::command]
pub fn schemas_export(
    state: State<'_, AppState>,
    schema_id: String,
    dest: String,
) -> Result<String> {
    let dest_path = PathBuf::from(&dest);
    state.schemas().export(&schema_id, &dest_path)?;
    Ok(dest)
}

// ================================================================ 설정

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
pub fn settings_set(state: State<'_, AppState>, settings: Settings) -> Result<()> {
    settings.save(&state.db)
}

/// 경로가 실제로 존재하고 FASTA 로 보이는 파일이 몇 개인지 알려준다.
/// 새 작업 폼이 "이 폴더에 61개 파일" 처럼 즉시 피드백하는 데 쓴다.
#[tauri::command]
pub fn inspect_input_dir(path: String) -> Result<InputDirInfo> {
    let dir = Path::new(&path);
    crate::paths::validate_host_path(dir)?;
    if !dir.is_dir() {
        return Err(Error::InvalidInput("폴더를 찾을 수 없습니다".into()));
    }

    const FASTA_EXT: [&str; 6] = ["fasta", "fa", "fna", "ffn", "faa", "frn"];
    let mut total = 0usize;
    let mut fasta = 0usize;
    for entry in std::fs::read_dir(dir)?.flatten() {
        if !entry.path().is_file() {
            continue;
        }
        total += 1;
        let ext = entry
            .path()
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if FASTA_EXT.contains(&ext.as_str()) {
            fasta += 1;
        }
    }
    Ok(InputDirInfo {
        path,
        total_files: total,
        fasta_files: fasta,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDirInfo {
    pub path: String,
    pub total_files: usize,
    pub fasta_files: usize,
}
