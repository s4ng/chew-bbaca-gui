//! UI ↔ 코어 ↔ SQLite 사이를 오가는 값 타입.
//!
//! 이 파일에는 WSL·wslpath·PGID 같은 개념이 등장하지 않는다. 이식 경계(§4.1)
//! 위쪽은 플랫폼 중립이어야 하며, `work_dir` 처럼 백엔드가 채우는 필드도
//! 상위 계층에서는 불투명한 문자열로만 다룬다.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------- 모듈

/// GUI 가 노출하는 chewBBACA 모듈 (§10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Module {
    CreateSchema,
    AlleleCall,
    /// AlleleCall 결과에서 core genome 을 추린다. 입력이 **어셈블리 폴더가 아니라
    /// TSV 파일 하나**라 다른 두 모듈과 입력 모양이 다르다.
    ExtractCgMLST,
}

impl Module {
    /// `chewBBACA.py <이 값>` 으로 그대로 들어가는 CLI 하위 명령 이름.
    pub fn cli_name(self) -> &'static str {
        match self {
            Module::CreateSchema => "CreateSchema",
            Module::AlleleCall => "AlleleCall",
            Module::ExtractCgMLST => "ExtractCgMLST",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "CreateSchema" => Some(Module::CreateSchema),
            "AlleleCall" => Some(Module::AlleleCall),
            "ExtractCgMLST" => Some(Module::ExtractCgMLST),
            _ => None,
        }
    }

    /// 어셈블리 폴더를 입력으로 받는가. `false` 면 파일 하나만 스테이징한다.
    pub fn takes_input_dir(self) -> bool {
        !matches!(self, Module::ExtractCgMLST)
    }

    /// 결과 폴더가 반드시 있어야 하는가.
    ///
    /// CreateSchema 는 산출물인 스키마를 앱 저장소에 넣으므로 회수할 것이 없다.
    /// 그런데도 폴더를 필수로 받으면 사용자는 **빈 폴더를 열어보고 실패했다고 생각한다.**
    /// 그래서 선택으로 두고, 지정한 경우에만 실행 로그 사본을 남긴다.
    pub fn requires_output_dir(self) -> bool {
        !matches!(self, Module::CreateSchema)
    }
}

// ---------------------------------------------------------------- 상태

/// §6.1 의 상태 모델. 종료 상태에서는 다른 상태로 전이하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "queued" => JobStatus::Queued,
            "running" => JobStatus::Running,
            "completed" => JobStatus::Completed,
            "cancelled" => JobStatus::Cancelled,
            _ => JobStatus::Failed,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        )
    }
}

// ---------------------------------------------------------------- 작업 정의

/// 새 작업 폼이 그대로 직렬화된 형태. 경로는 모두 **Windows 경로**다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSpec {
    pub module: Module,
    /// 어셈블리(FASTA) 폴더
    pub input_dir: String,
    /// 결과를 회수할 Windows 폴더
    pub output_dir: String,
    /// AlleleCall: 사용할 스키마. CreateSchema: 만들어질 스키마 이름의 원본.
    pub schema_id: Option<String>,
    /// CreateSchema 로 새로 만들 스키마의 표시 이름
    pub schema_name: Option<String>,
    /// CreateSchema 용 Prodigal training file (Windows 경로). 생략 가능.
    pub ptf: Option<String>,
    /// 입력이 이미 CDS 인 경우 `--cds`
    #[serde(default)]
    pub cds_input: bool,
    /// 일부 loci 만 대상으로 할 때 (`--gl`) 사용할 목록 파일 (Windows 경로)
    pub loci_list: Option<String>,
    /// 미지정 시 WSL 내부 `nproc` 값을 사용한다 (§6.4)
    pub cpu: Option<u32>,
    /// ExtractCgMLST 입력: AlleleCall 이 만든 `results_alleles.tsv` (Windows 경로).
    /// 이 모듈은 `input_dir` 을 쓰지 않는다.
    #[serde(default)]
    pub profiles_file: Option<String>,
    /// ExtractCgMLST 의 `--t`. 비우면 chewBBACA 기본값(0.95 / 0.99 / 1)을 모두 계산한다.
    #[serde(default)]
    pub thresholds: Option<String>,
}

/// SQLite `jobs` 한 행. UI 목록/상세가 그대로 사용한다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub job_id: String,
    pub module: Module,
    pub status: JobStatus,
    /// 원본 `JobSpec` 의 JSON. 재현과 이력 표시에 쓴다.
    pub args: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    /// 취소·고아 판정에 쓰는 프로세스 그룹 ID (§6.2)
    pub pgid: Option<i32>,
    /// 백엔드 내부 작업 디렉터리 (상위 계층에는 불투명)
    pub work_dir: Option<String>,
    /// 로그는 파일로, DB 에는 경로만 (§6.1)
    pub log_path: Option<String>,
    pub output_path: Option<String>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub progress: Option<f32>,
}

// ---------------------------------------------------------------- 스키마

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaInfo {
    pub schema_id: String,
    pub name: String,
    pub created_at: String,
    /// 생성한 작업 (추적용)
    pub created_by_job: Option<String>,
    /// 앱이 소유하는 백엔드 내부 경로 (§4.4)
    pub backend_path: String,
    /// 함께 보관되는 Prodigal training file. AlleleCall 마다 동일한 것을 써야 한다.
    pub ptf: Option<String>,
    pub loci_count: Option<i64>,
}

// ---------------------------------------------------------------- 이벤트

/// `job://log` — 한 줄씩 방출한다. 파일 기록과 동시에 UI 로 간다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub job_id: String,
    pub stream: LogStream,
    pub line: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogStream {
    Stdout,
    Stderr,
    /// 앱 자신이 남기는 안내 (스테이징 시작, 결과 회수 등)
    App,
}

/// `job://state` — 상태 전이 알림. UI 는 이 이벤트로 목록을 갱신한다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateEvent {
    pub job_id: String,
    pub status: JobStatus,
    pub message: Option<String>,
}

/// `job://progress` — chewBBACA 출력에서 파싱한 진행률 (0.0 ~ 1.0).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub job_id: String,
    pub fraction: f32,
    pub label: String,
}

pub const EVENT_LOG: &str = "job://log";
pub const EVENT_STATE: &str = "job://state";
pub const EVENT_PROGRESS: &str = "job://progress";
