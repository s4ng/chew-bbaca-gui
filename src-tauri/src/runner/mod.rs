//! 이식 경계 (ARCHITECTURE.md §4.1).
//!
//! **이 선 위쪽은 플랫폼 중립이다.** `wslpath`, `wsl --import`, PGID 같은 개념이
//! 이 모듈 밖으로 새어나가면 안 된다. macOS 확장 시 교체되는 것은 아래쪽뿐이다(§9).
//!
//! 문서의 스케치와 다른 점이 하나 있다 — `run()` 이 `JobHandle` 을 즉시 돌려주지
//! 않고 **완료까지 블로킹하며 `EventSink` 로 스트리밍**한다. Job Manager 가 작업당
//! 스레드를 하나 잡고 있으므로 이 형태가 더 단순하고, PGID 는 획득 즉시
//! `RunEvent::Pgid` 로 올려보내므로 취소 가능 시점은 동일하다.

pub mod cli;
pub mod progress;
pub mod wsl;

use std::path::Path;
use std::sync::Arc;

use crate::error::Result;
use crate::models::JobSpec;

/// 취소·고아 판정에 필요한 최소 정보.
#[derive(Debug, Clone)]
pub struct JobHandle {
    pub job_id: String,
    /// 백엔드 내부 작업 디렉터리. 상위 계층에는 불투명한 문자열이다.
    pub work_dir: String,
    /// 프로세스 그룹 ID. 획득 전에는 `None` 이며 이때는 취소가 불가능하다.
    pub pgid: Option<i32>,
}

/// 실행 중 백엔드가 올려보내는 신호.
#[derive(Debug, Clone)]
pub enum RunEvent {
    /// 프로세스 그룹 생성 직후. Job Manager 는 **받자마자 DB 에 기록**해야 한다.
    Pgid(i32),
    Stdout(String),
    Stderr(String),
    /// 앱 자신의 안내 ("입력 복사 중...", "결과 회수 중...")
    Notice(String),
}

pub type EventSink = Arc<dyn Fn(RunEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub exit_code: i32,
    pub work_dir: String,
    /// 결과를 회수한 Windows 경로 (회수하지 않았으면 `None`)
    pub collected_to: Option<String>,
    /// CreateSchema 가 만든 스키마의 백엔드 경로
    pub created_schema: Option<CreatedSchema>,
}

#[derive(Debug, Clone)]
pub struct CreatedSchema {
    pub schema_id: String,
    pub name: String,
    pub backend_path: String,
    pub ptf: Option<String>,
    pub loci_count: Option<i64>,
}

/// 백엔드 준비 상태 요약. 온보딩 화면이 그대로 표시한다.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendStatus {
    pub ready: bool,
    pub distro: String,
    pub chewbbaca_version: Option<String>,
    pub cpu_count: Option<u32>,
    pub detail: String,
}

pub trait ChewieRunner: Send + Sync {
    /// 백엔드가 실행 가능한지 확인한다. 실패는 온보딩 게이트로 되돌리는 신호다.
    fn ensure_ready(&self) -> Result<()>;

    fn status(&self) -> BackendStatus;

    /// 호스트 경로 → 백엔드 경로. 문자열 치환을 직접 구현하지 않는다.
    fn to_backend_path(&self, host: &Path) -> Result<String>;

    /// `--cpu` 인자에 넣을 값. Windows 논리 코어 수와 다를 수 있다 (§6.4).
    fn cpu_count(&self) -> Result<u32>;

    /// 완료될 때까지 블로킹한다. 로그·PGID 는 `sink` 로 흘려보낸다.
    fn run(&self, job_id: &str, spec: &JobSpec, sink: &EventSink) -> Result<RunOutcome>;

    /// 프로세스 그룹 전체를 종료한다 (§6.2).
    fn cancel(&self, handle: &JobHandle) -> Result<()>;

    /// 앱 시작 시 조정(reconciliation)용 생존 확인 (§6.3).
    fn is_alive(&self, handle: &JobHandle) -> Result<bool>;

    /// 산출물이 실제로 만들어졌는지. 죽은 작업을 `completed` 로 확정할지
    /// `failed` 로 표시할지 가르는 유일한 근거다 (§6.3).
    fn output_populated(&self, job_id: &str) -> Result<bool>;

    /// 앱이 소유하는 스키마 디렉터리 목록.
    fn list_schema_dirs(&self) -> Result<Vec<String>>;

    /// 스키마 ID → 백엔드 경로. DB 가 유실되어도 규칙으로 재구성할 수 있어야 한다.
    fn schema_path(&self, schema_id: &str) -> Result<String>;

    fn remove_schema(&self, schema_id: &str) -> Result<()>;

    /// 백엔드 디렉터리를 Windows 폴더로 내보낸다.
    fn export_dir(&self, backend_path: &str, host_dest: &Path) -> Result<()>;

    /// 완료된 작업의 임시 공간을 비운다.
    fn cleanup_work(&self, job_id: &str) -> Result<()>;
}
