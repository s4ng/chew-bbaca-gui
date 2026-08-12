//! Tauri IPC 표면 (§2 의 `Presentation ↔ Application Core` 경계).
//!
//! 규칙 하나만 지키면 된다 — **여기서 WSL 이라는 단어가 프런트로 넘어가지
//! 않게 한다.** `env_*` 계열은 예외다. 온보딩은 본질적으로 Windows/WSL 절차라
//! UI 가 그 사실을 알아야 하며, macOS 확장 시 통째로 교체될 화면들이다(§9).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::api;
use crate::db::Db;
use crate::env::probe::firmware_hint;
use crate::env::{DownloadProgress, EnvReport, Provisioner, RootfsOrigin};
use crate::error::{Error, Result};
use crate::jobs::JobManager;
use crate::mcp::{McpServer, McpStatus};
use crate::models::{Job, JobSpec, SchemaInfo};
use crate::paths::AppPaths;
use crate::runner::BackendStatus;
use crate::schema_store::SchemaStore;
use crate::settings::{McpSettings, Settings};

/// 온보딩 ③ 단계 진행 상황. 다운로드 진행률과 단계 전환을 함께 싣는다.
pub const EVENT_PROVISION: &str = "env://provision";

pub struct AppState {
    pub db: Arc<Db>,
    pub paths: AppPaths,
    pub manager: Arc<JobManager>,
    /// Tauri 리소스 디렉터리. 동봉된 rootfs 가 여기 들어 있다 (§8.1).
    /// 개발 실행에서는 리소스가 복사되지 않으므로 파일이 없는 것이 정상이다.
    pub resources: Option<PathBuf>,
    /// 로컬 MCP 서버 (`doc/MCP.md`). 앱과 수명을 같이 한다.
    pub mcp: Arc<McpServer>,
}

impl AppState {
    /// `api.rs` 와 `mcp/` 도 부른다 — 설정은 언제나 DB 가 진실이므로 매번 읽는다.
    pub(crate) fn settings(&self) -> Settings {
        Settings::load(&self.db)
    }

    pub(crate) fn provisioner(&self) -> Provisioner {
        Provisioner::new(self.paths.clone(), self.settings().distro)
            .with_resources(self.resources.clone())
    }

    pub(crate) fn schemas(&self) -> SchemaStore {
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

/// ExtractCgMLST 입력 파일 진단 결과. 폼이 즉시 피드백하는 데 쓴다.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilesInfo {
    /// 헤더를 뺀 행 수 = 균주 수
    pub genomes: usize,
    /// 첫 열을 뺀 열 수 = loci 수
    pub loci: usize,
    /// 첫 열의 머리말. AlleleCall 결과라면 `FILE` 이다.
    pub first_column: String,
    pub looks_valid: bool,
}

/// `--gl` loci 목록 파일 진단 결과.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LociListInfo {
    pub looks_valid: bool,
    /// 빈 줄을 뺀 줄 수 = 대상 loci 수
    pub loci: usize,
    /// 탭이 있으면 loci 목록이 아니라 표다.
    pub tabbed: bool,
    pub first_line: String,
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
    api::env_probe(state.inner())
}

/// 백엔드(배포판 + chewBBACA) 상태. 게이트 통과 후 상세 확인용.
#[tauri::command]
pub fn backend_status(state: State<'_, AppState>) -> BackendStatus {
    api::backend_status(state.inner())
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

/// rootfs 를 어디서 가져오게 되는지. UI 가 문구와 버튼을 이 값으로 고른다.
#[tauri::command]
pub fn env_rootfs_origin(state: State<'_, AppState>) -> RootfsOrigin {
    state.provisioner().rootfs_origin(&state.settings().rootfs)
}

/// 배포판 게이트 ③ — rootfs 확보 → SHA256 검증 → `wsl --import`.
///
/// 동봉본이라도 500MB 해싱에 수 초가 걸리므로 별도 스레드에서 돌리고 진행 상황은
/// `env://provision` 이벤트로 보낸다.
#[tauri::command]
pub fn env_provision(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let settings = state.settings();
    let provisioner = state.provisioner();
    let origin = provisioner.rootfs_origin(&settings.rootfs);

    if origin == RootfsOrigin::Missing {
        return Err(Error::Other(format!(
            "설치할 rootfs 이미지를 찾을 수 없습니다.\n인스톨러로 설치한 앱이라면 다시 설치해 주세요. 개발 중이라면 설정 화면의 [rootfs 이미지] 칸에 직접 빌드한 {} 경로를 넣으세요.",
            settings.rootfs.file_name
        )));
    }
    if !settings.checksum_looks_valid() {
        return Err(Error::Other(
            "설정의 SHA256 값이 64자리 16진수가 아닙니다. 설정 화면에서 체크섬을 확인해 주세요."
                .into(),
        ));
    }

    // 동봉본·로컬 파일은 받지 않고 해싱만 한다. 첫 이벤트 문구가 다르면
    // 사용자가 "왜 다운로드가 안 끝나지" 라고 오해한다.
    let downloading = origin == RootfsOrigin::Remote;
    let source = settings.rootfs.clone();

    std::thread::spawn(move || {
        let emit =
            |stage: &'static str, message: String, fraction: Option<f32>, ok: Option<bool>| {
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

        // 다운로드 단계와 검증 단계는 같은 진행률 콜백을 쓴다 (둘 다 바이트 단위 진행).
        let stage = if downloading { "download" } else { "verify" };
        let first = if downloading {
            "rootfs 다운로드를 시작합니다"
        } else {
            "포함된 이미지를 검증합니다"
        };
        emit(stage, first.into(), Some(0.0), None);

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
                    stage,
                    message,
                    fraction,
                    ok: None,
                },
            );
        };

        let tarball = match provisioner.download_rootfs(&source, &on_progress) {
            Ok(p) => p,
            Err(e) => {
                emit(stage, e.to_string(), None, Some(false));
                return;
            }
        };

        emit("verify", "체크섬 검증 완료".into(), None, None);
        emit("import", "배포판을 등록하는 중...".into(), None, None);

        match provisioner.import_distro(&tarball) {
            Ok(()) => emit(
                "done",
                "환경 구성이 완료되었습니다".into(),
                None,
                Some(true),
            ),
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
    api::disk_usage(state.inner())
}

// ================================================================ 작업

#[tauri::command]
pub fn jobs_submit(state: State<'_, AppState>, spec: JobSpec) -> Result<String> {
    api::jobs_submit(state.inner(), spec)
}

#[tauri::command]
pub fn jobs_list(state: State<'_, AppState>, limit: Option<i64>) -> Result<Vec<Job>> {
    api::jobs_list(state.inner(), limit.unwrap_or(100))
}

#[tauri::command]
pub fn jobs_get(state: State<'_, AppState>, job_id: String) -> Result<Option<Job>> {
    api::jobs_get(state.inner(), &job_id)
}

#[tauri::command]
pub fn jobs_cancel(state: State<'_, AppState>, job_id: String) -> Result<()> {
    api::jobs_cancel(state.inner(), &job_id)
}

/// 로그 파일 전체. UI 가 이벤트를 놓쳤거나 앱을 다시 켠 경우 이걸로 복원한다.
#[tauri::command]
pub fn jobs_log(state: State<'_, AppState>, job_id: String) -> Result<String> {
    api::jobs_log(state.inner(), &job_id)
}

/// 앱 시작 시 조정 (§6.3). 살아 있는 작업 목록을 돌려주면 UI 가
/// "이전 작업이 실행 중입니다 — 복구 / 종료" 를 띄운다.
#[tauri::command]
pub fn jobs_reconcile(state: State<'_, AppState>) -> Result<Vec<Job>> {
    state.manager.reconcile()
}

/// 이어받은 작업 중 아직 실행 중인 것. 조정과 달리 **몇 번이고 물어도 된다.**
#[tauri::command]
pub fn jobs_adopted(state: State<'_, AppState>) -> Result<Vec<Job>> {
    state.manager.adopted_jobs()
}

/// 평가 리포트 HTML 을 기본 브라우저로 연다. 실제 동작은 `api::report_open` 에 있다
/// (MCP 도구도 같은 함수를 부른다).
#[tauri::command]
pub fn report_open(app: AppHandle, state: State<'_, AppState>, job_id: String) -> Result<String> {
    api::report_open(&app, state.inner(), &job_id)
}

// ================================================================ 스키마

#[tauri::command]
pub fn schemas_list(state: State<'_, AppState>) -> Result<Vec<SchemaInfo>> {
    api::schemas_list(state.inner())
}

#[tauri::command]
pub fn schemas_delete(state: State<'_, AppState>, schema_id: String) -> Result<()> {
    state.schemas().delete(&schema_id)
}

/// 내보낸 스키마 폴더를 다시 들여온다. 되돌릴 수 있는 조작이라 확인을 받지 않는다.
#[tauri::command]
pub fn schemas_import(state: State<'_, AppState>, dir: String, name: String) -> Result<SchemaInfo> {
    state.schemas().import(Path::new(&dir), &name)
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

// ================================================================ MCP

/// 로컬 MCP 서버의 현재 상태. 설정 화면이 그대로 표시한다 (`doc/MCP.md` §7).
#[tauri::command]
pub fn mcp_status(state: State<'_, AppState>) -> McpStatus {
    state.mcp.status(state.inner())
}

/// MCP 설정을 바꾸고 서버를 다시 띄운다.
///
/// **`settings_set` 으로는 리스너가 갱신되지 않는다.** 포트나 켬/끔이 바뀌면
/// 실제로 열린 소켓을 바꿔야 하므로 전용 명령을 둔다.
#[tauri::command]
pub fn mcp_configure(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
    port: u16,
    allow_run: bool,
) -> Result<McpStatus> {
    let mut settings = state.settings();
    settings.mcp.enabled = enabled;
    settings.mcp.port = port;
    settings.mcp.allow_run = allow_run;
    settings.save(&state.db)?;

    state.mcp.stop();
    if enabled {
        state.mcp.start(&app)?;
    }
    Ok(state.mcp.status(state.inner()))
}

/// 토큰을 새로 발급한다. **기존 클라이언트 설정은 즉시 무효가 된다** —
/// UI 에서 확인을 받은 뒤 호출하고, 새 설정 조각을 다시 안내해야 한다.
#[tauri::command]
pub fn mcp_regenerate_token(app: AppHandle, state: State<'_, AppState>) -> Result<McpStatus> {
    let mut settings = state.settings();
    settings.mcp.token = McpSettings::new_token();
    settings.save(&state.db)?;

    // 토큰은 요청마다 DB 에서 읽으므로 재기동이 꼭 필요하지는 않지만,
    // 접속 정보 파일(mcp.json)을 새 값으로 다시 쓰기 위해 한 번 돌린다.
    state.mcp.stop();
    if settings.mcp.enabled {
        state.mcp.start(&app)?;
    }
    Ok(state.mcp.status(state.inner()))
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

/// `--gl` 에 넘길 loci 목록 파일인지 확인한다.
///
/// 프로파일 표(가로로 넓다)를 여기에 잘못 넣는 것이 흔한 실수다. loci 목록은
/// **한 줄에 식별자 하나**이므로 탭이 들어 있으면 표로 보고 거른다.
/// 정상이라면 몇 개를 대상으로 하게 되는지 알려준다 — 3,127 중 1,270 처럼
/// 숫자가 보이면 사용자가 잘못 골랐는지 스스로 알아챈다.
#[tauri::command]
pub fn inspect_loci_list(path: String) -> Result<LociListInfo> {
    use std::io::BufRead;

    let file = Path::new(&path);
    crate::paths::validate_host_path(file)?;
    if !file.is_file() {
        return Err(Error::InvalidInput("파일을 찾을 수 없습니다".into()));
    }

    let reader = std::io::BufReader::new(std::fs::File::open(file)?);
    let mut loci = 0usize;
    let mut tabbed = false;
    let mut first = String::new();

    for line in reader.lines().map_while(std::result::Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if first.is_empty() {
            first = trimmed.to_string();
        }
        if trimmed.contains('\t') {
            tabbed = true;
        }
        loci += 1;
    }

    Ok(LociListInfo {
        looks_valid: loci > 0 && !tabbed,
        loci,
        tabbed,
        first_line: first,
    })
}

/// 따라해보기 가이드를 앱 폴더에 꺼내 기본 브라우저로 연다.
///
/// 문서와 스크린샷이 **바이너리에 묻어서 나가므로 인터넷이 없어도** 열린다.
/// Tauri 리소스로 동봉하지 않는 이유는 개발 실행에서는 리소스가 복사되지 않아
/// 경로가 갈리기 때문이다. 매번 덮어써서 앱을 새로 깔면 문서도 함께 갱신된다.
///
/// **여는 것까지 Rust 가 한다.** 프런트에서 `openPath` 를 부르면 열리지 않는다 —
/// `opener:allow-open-path` 는 "스코프 없이 명령만 허용"이라 어떤 경로도 통과하지
/// 못하기 때문이다. 우리가 방금 쓴 파일을 우리가 여는 것이므로 여기서 처리한다.
#[tauri::command]
pub fn guide_open(app: AppHandle, state: State<'_, AppState>) -> Result<String> {
    // (파일명, 내용) — 스크린샷은 HTML 이 상대 경로로 참조하므로 같은 폴더에 푼다.
    const HTML: &str = include_str!("../guide/guide.html");
    const SHOTS: [(&str, &[u8]); 3] = [
        ("01-new-job.png", include_bytes!("../guide/01-new-job.png")),
        (
            "02-job-running.png",
            include_bytes!("../guide/02-job-running.png"),
        ),
        ("03-schemas.png", include_bytes!("../guide/03-schemas.png")),
    ];

    open_guide(&app, state.inner(), "따라해보기.html", HTML, &SHOTS)
}

/// MCP 연결 안내. 설정 화면의 [MCP 서버] 칸에서 연다.
///
/// 따라해보기와 나누어 둔 이유는 읽는 시점이 다르기 때문이다 — 이쪽은 분석을 이미
/// 할 줄 아는 사람이 "대화로 시키고 싶을 때" 한 번 보는 문서다.
#[tauri::command]
pub fn mcp_guide_open(app: AppHandle, state: State<'_, AppState>) -> Result<String> {
    const HTML: &str = include_str!("../guide/mcp.html");
    const SHOTS: [(&str, &[u8]); 4] = [
        (
            "mcp-01-app-settings.png",
            include_bytes!("../guide/mcp-01-app-settings.png"),
        ),
        (
            "mcp-02-chatgpt-settings.png",
            include_bytes!("../guide/mcp-02-chatgpt-settings.png"),
        ),
        (
            "mcp-04-server-list.png",
            include_bytes!("../guide/mcp-04-server-list.png"),
        ),
        (
            "mcp-05-headers.png",
            include_bytes!("../guide/mcp-05-headers.png"),
        ),
    ];

    open_guide(&app, state.inner(), "MCP 연결하기.html", HTML, &SHOTS)
}

/// 가이드 일습을 앱 폴더에 풀고 기본 브라우저로 연다.
///
/// **문서와 스크린샷이 바이너리에 묻어서 나가므로 인터넷이 없어도** 열린다.
/// Tauri 리소스로 동봉하지 않는 이유는 개발 실행에서는 리소스가 복사되지 않아
/// 경로가 갈리기 때문이다. 매번 덮어써서 앱을 새로 깔면 문서도 함께 갱신된다.
///
/// **여는 것까지 Rust 가 한다.** 프런트에서 `openPath` 를 부르면 열리지 않는다 —
/// `opener:allow-open-path` 는 "스코프 없이 명령만 허용"이라 어떤 경로도 통과하지
/// 못하기 때문이다. 우리가 방금 쓴 파일을 우리가 여는 것이므로 여기서 처리한다.
fn open_guide(
    app: &AppHandle,
    state: &AppState,
    file_name: &str,
    html: &str,
    shots: &[(&str, &[u8])],
) -> Result<String> {
    use tauri_plugin_opener::OpenerExt;

    let dir = state.paths.root.join("guide");
    std::fs::create_dir_all(&dir)?;
    for (name, bytes) in shots {
        std::fs::write(dir.join(name), bytes)?;
    }
    let path = dir.join(file_name);
    std::fs::write(&path, html)?;

    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| {
            Error::Other(format!(
                "가이드를 열지 못했습니다: {e}\n파일은 여기 있습니다: {}",
                path.display()
            ))
        })?;
    Ok(path.to_string_lossy().to_string())
}

/// AlleleCall 결과 표인지 확인한다 (ExtractCgMLST 입력 게이트).
///
/// AlleleCall 결과 폴더에는 TSV 가 일곱 개 들어 있고 확장자만으로는 구별되지 않는다.
/// 엉뚱한 것을 넣어도 chewBBACA 는 **거절하지 않고** 각 행을 균주로 취급해 한참을
/// 돌다가 쓸모없는 결과를 낸다 (`cds_coordinates.tsv` 로 64,217 행을 도는 사례가 있었다).
/// 그래서 제출 전에 여기서 거른다 — 40분 뒤에 알게 되는 일이 없어야 한다 (§5.4).
#[tauri::command]
pub fn inspect_profiles_file(path: String) -> Result<ProfilesInfo> {
    use std::io::BufRead;

    let file = Path::new(&path);
    crate::paths::validate_host_path(file)?;
    if !file.is_file() {
        return Err(Error::InvalidInput("파일을 찾을 수 없습니다".into()));
    }

    let handle = std::fs::File::open(file)?;
    let mut reader = std::io::BufReader::new(handle);

    let mut header = String::new();
    reader.read_line(&mut header)?;
    let columns = header.trim_end().split('\t').count();
    let first_column = header
        .split('\t')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();

    // 헤더를 뺀 나머지가 균주 수다. 4MB 짜리 오답 파일도 순식간에 세어진다.
    let rows = reader.lines().map_while(std::result::Result::ok).count();

    // AlleleCall 결과는 첫 칸이 `FILE` 이고 loci 마다 열이 하나씩 붙는다.
    // 열이 한 자릿수면 프로파일 표일 수 없다.
    let looks_valid = first_column.eq_ignore_ascii_case("FILE") && columns > 10;

    Ok(ProfilesInfo {
        genomes: rows,
        loci: columns.saturating_sub(1),
        first_column,
        looks_valid,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDirInfo {
    pub path: String,
    pub total_files: usize,
    pub fasta_files: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 헤더 한 줄과 데이터 `rows` 줄짜리 TSV 를 임시로 만든다.
    fn write_tsv(name: &str, header: &str, rows: usize) -> PathBuf {
        let path = std::env::temp_dir().join(format!("chewie-gate-{name}"));
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "{header}").unwrap();
        let cols = header.split('\t').count();
        for i in 0..rows {
            let line: Vec<String> = (0..cols).map(|c| format!("v{i}_{c}")).collect();
            writeln!(f, "{}", line.join("\t")).unwrap();
        }
        path
    }

    // 리포트 탐색(`find_report`) 테스트는 `api.rs` 로 함께 옮겼다.

    #[test]
    fn allelic_profile_table_is_accepted() {
        // results_alleles.tsv 모양 — 첫 열 FILE, loci 마다 열 하나, 균주마다 행 하나.
        let header = std::iter::once("FILE".to_string())
            .chain((1..=40).map(|i| format!("locus{i}")))
            .collect::<Vec<_>>()
            .join("\t");
        let path = write_tsv("alleles.tsv", &header, 32);

        let info = inspect_profiles_file(path.to_string_lossy().to_string()).unwrap();
        assert!(info.looks_valid);
        assert_eq!(info.genomes, 32);
        assert_eq!(info.loci, 40);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn cds_coordinates_table_is_rejected() {
        // 실제로 물렸던 파일의 모양. 이걸 통과시키면 64,217 행을 도는 사고가 재현된다.
        let path = write_tsv(
            "coords.tsv",
            "Genome\tContig\tStart\tStop\tProtein_ID\tCoding_Strand",
            500,
        );
        let info = inspect_profiles_file(path.to_string_lossy().to_string()).unwrap();
        assert!(!info.looks_valid, "이 표는 거절해야 한다");
        assert_eq!(info.first_column, "Genome");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_loci_list_is_accepted_and_counted() {
        // ExtractCgMLST 가 만드는 cgMLSTschema*.txt 모양 — 한 줄에 식별자 하나.
        let path = std::env::temp_dir().join("chewie-gate-loci.txt");
        std::fs::write(
            &path,
            "genome1-protein1
genome1-protein10

genome1-protein11
",
        )
        .unwrap();
        let info = inspect_loci_list(path.to_string_lossy().to_string()).unwrap();
        assert!(info.looks_valid);
        assert_eq!(info.loci, 3, "빈 줄은 세지 않는다");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_profile_table_is_rejected_as_a_loci_list() {
        // 프로파일 표를 --gl 에 넣는 것이 흔한 실수다. 탭이 있으면 표로 본다.
        let path = std::env::temp_dir().join("chewie-gate-loci-tsv.txt");
        std::fs::write(
            &path,
            "FILE	locus1	locus2
g1	1	2
",
        )
        .unwrap();
        let info = inspect_loci_list(path.to_string_lossy().to_string()).unwrap();
        assert!(!info.looks_valid);
        assert!(info.tabbed);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_table_with_too_few_columns_is_rejected() {
        // 첫 열이 FILE 이어도 열이 한 자릿수면 프로파일 표일 수 없다.
        let path = write_tsv("tiny.tsv", "FILE\ta\tb", 3);
        let info = inspect_profiles_file(path.to_string_lossy().to_string()).unwrap();
        assert!(!info.looks_valid);
        let _ = std::fs::remove_file(path);
    }
}
