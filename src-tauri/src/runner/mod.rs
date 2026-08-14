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

/// CreateSchema 가 만들 스키마의 ID. **작업 ID 와 이름만으로 결정된다.**
///
/// 실행할 때와 나중에 조정할 때가 같은 값을 얻어야 하므로 규칙을 한곳에 둔다.
/// 러너 구현체가 바뀌어도 이 규칙은 그대로다.
pub fn schema_id_for(job_id: &str, spec: &JobSpec) -> String {
    format!(
        "{}-{}",
        crate::util::slugify(schema_name_of(spec)),
        &job_id[..8.min(job_id.len())]
    )
}

/// CreateSchema 가 만들 스키마의 표시 이름.
///
/// 빈 이름은 `submit()` 이 막지만, 여기서도 무너지지 않게 기본값을 둔다 —
/// 작업 ID 를 이름으로 쓰면 `abcd1234-0000-…-abcd1234` 같은 것이 만들어진다.
pub fn schema_name_of(spec: &JobSpec) -> &str {
    match &spec.params {
        crate::models::ModuleParams::CreateSchema { schema_name, .. }
        | crate::models::ModuleParams::PrepExternalSchema { schema_name, .. }
            if !schema_name.trim().is_empty() =>
        {
            schema_name
        }
        _ => "schema",
    }
}

#[derive(Debug, Clone)]
pub struct CreatedSchema {
    pub schema_id: String,
    pub name: String,
    pub backend_path: String,
    pub ptf: Option<String>,
    pub loci_count: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Module;

    fn spec(name: &str) -> JobSpec {
        JobSpec {
            output_dir: String::new(),
            cpu: None,
            params: crate::models::ModuleParams::CreateSchema {
                input_dir: String::new(),
                schema_name: name.to_string(),
                ptf: None,
                cds_input: false,
            },
        }
    }

    #[test]
    fn schema_id_is_stable_for_the_same_job() {
        // 실행할 때와 조정할 때가 같은 값을 얻어야 고아 작업의 산출물을 찾을 수 있다.
        let job = "c8bb38f8-52ba-4450-adf5-99a88d9d0630";
        let a = schema_id_for(job, &spec("test123"));
        let b = schema_id_for(job, &spec("test123"));
        assert_eq!(a, b);
        assert_eq!(a, "test123-c8bb38f8");
    }

    #[test]
    fn schema_id_falls_back_when_name_is_blank() {
        // 폼과 submit() 이 빈 이름을 막지만, 여기서도 쓸 만한 ID 가 나와야 한다.
        let job = "abcd1234-0000-0000-0000-000000000000";
        assert_eq!(schema_id_for(job, &spec("   ")), "schema-abcd1234");
    }

    #[test]
    fn module_is_derived_from_params() {
        // 별도의 module 필드가 없으므로 둘이 어긋날 수 없다.
        assert_eq!(spec("x").module(), Module::CreateSchema);
        assert_eq!(spec("x").input_dir(), Some(""));
    }
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
    ///
    /// **`spec` 이 필요한 이유:** 산출물이 가는 곳이 모듈마다 다르다. CreateSchema 는
    /// 작업 디렉터리가 아니라 스키마 저장소에 결과를 남기므로, 작업 디렉터리만 보면
    /// 성공한 작업도 실패로 확정된다.
    fn output_produced(&self, job_id: &str, spec: &JobSpec) -> Result<bool>;

    /// 앱이 소유하는 스키마 디렉터리 목록.
    fn list_schema_dirs(&self) -> Result<Vec<String>>;

    /// 이미 있는 스키마 디렉터리를 조사한다 (loci 수, training file).
    ///
    /// 두 곳에서 쓴다 — 이어받은 작업이 끝났을 때의 등록, 그리고 DB 가 유실된 뒤의 복구.
    /// 둘 다 `RunOutcome` 을 손에 쥐지 못한 상황이라 디렉터리를 직접 봐야 한다.
    fn inspect_schema_dir(&self, schema_id: &str, name: &str) -> Result<CreatedSchema>;

    /// 스키마 ID → 백엔드 경로. DB 가 유실되어도 규칙으로 재구성할 수 있어야 한다.
    fn schema_path(&self, schema_id: &str) -> Result<String>;

    fn remove_schema(&self, schema_id: &str) -> Result<()>;

    /// Windows 폴더의 스키마를 앱 저장소로 들여온다 (내보내기의 역방향).
    ///
    /// 원본은 손대지 않고 복사만 한다. 같은 ID 가 이미 있으면 덮어쓰지 않고 실패한다 —
    /// 사용 중인 스키마를 조용히 갈아치우면 이전 AlleleCall 결과와 대응이 깨진다.
    fn import_schema_dir(&self, host_src: &Path, schema_id: &str) -> Result<String>;

    /// 백엔드 디렉터리를 Windows 폴더로 내보낸다.
    fn export_dir(&self, backend_path: &str, host_dest: &Path) -> Result<()>;

    /// 참조 게놈 하나로 Prodigal training file 을 만든다.
    ///
    /// **작업(Job)이 아니라 동기 호출이다.** 게놈 하나 학습은 수십 초면 끝나고
    /// 산출물이 파일 하나뿐이라, PGID·취소·고아 복구 기계장치가 전부 놀게 된다.
    ///
    /// 이름에 구현(pyrodigal)이 드러나지 않는 것은 의도적이다 — 그것은 경계
    /// 아래의 사실이고, macOS 러너가 생기면 다른 것을 부를 수도 있다.
    fn create_training_file(&self, host_genome: &Path, host_output: &Path) -> Result<()>;

    /// 완료된 작업의 임시 공간을 비운다.
    fn cleanup_work(&self, job_id: &str) -> Result<()>;
}
