//! Application Core 조립부 (ARCHITECTURE.md §2).
//!
//! 여기서 하는 일은 세 가지뿐이다 — 디렉터리 준비, 컴포넌트 배선, IPC 등록.
//! 실제 로직은 각 모듈에 있고, 이 파일은 "무엇이 무엇에 의존하는가" 만 보여준다.
//!
//! ```text
//! JobManager ──▶ ChewieRunner (WslRunner)   ◀── 이식 경계
//!     │
//!     ├──▶ Db (SQLite)
//!     └──▶ AppHandle (이벤트 방출)
//! ```

mod commands;
mod db;
mod env;
mod error;
mod jobs;
mod models;
mod paths;
mod runner;
mod schema_store;
mod settings;
mod util;
mod win;

use std::sync::Arc;

use tauri::Manager;

use crate::commands::AppState;
use crate::db::Db;
use crate::jobs::JobManager;
use crate::paths::AppPaths;
use crate::runner::wsl::WslRunner;
use crate::runner::ChewieRunner;
use crate::settings::Settings;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let paths = AppPaths::resolve()?;
            paths.ensure_dirs()?;

            let db = Arc::new(Db::open(&paths.db)?);
            let settings = Settings::load(&db);

            // 이식 경계는 이 한 줄이다. macOS 지원 시 여기서 NativeRunner 를
            // 고르게 되고, 위쪽 코드는 바뀌지 않는다 (§9).
            let runner: Arc<dyn ChewieRunner> = Arc::new(WslRunner::new(settings.distro.clone()));

            let manager = JobManager::new(
                app.handle().clone(),
                Arc::clone(&db),
                runner,
                paths.clone(),
            );

            // 조정(reconciliation)은 여기서 자동 실행하지 않는다. 살아 있는 작업을
            // 발견하면 사용자에게 "복구 / 종료" 를 물어야 하므로 UI 가 준비된 뒤
            // `jobs_reconcile` 로 호출한다 (§6.3).
            // rootfs 는 인스톨러에 동봉되어 리소스 디렉터리에 놓인다 (§8.1).
            // 개발 실행에서는 리소스가 복사되지 않으므로 없는 것이 정상이다.
            let resources = app.path().resource_dir().ok();

            app.manage(AppState {
                db,
                paths,
                manager,
                resources,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::env_probe,
            commands::backend_status,
            commands::env_install_wsl,
            commands::env_manual_commands,
            commands::env_firmware_hint,
            commands::env_reboot_to_firmware,
            commands::env_rootfs_origin,
            commands::env_provision,
            commands::env_unregister,
            commands::disk_compact,
            commands::disk_usage,
            commands::jobs_submit,
            commands::jobs_list,
            commands::jobs_get,
            commands::jobs_cancel,
            commands::jobs_log,
            commands::jobs_reconcile,
            commands::jobs_adopted,
            commands::schemas_list,
            commands::schemas_delete,
            commands::schemas_export,
            commands::settings_get,
            commands::settings_set,
            commands::inspect_input_dir,
            commands::inspect_profiles_file,
            commands::guide_open,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 애플리케이션을 시작하지 못했습니다");
}
