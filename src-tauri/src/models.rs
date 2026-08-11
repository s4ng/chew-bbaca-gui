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
    /// 외부 스키마를 chewBBACA 형식으로 변환해 앱 저장소에 등록한다.
    /// CreateSchema 와 같은 자리(파이프라인 1단계)를 대신한다.
    PrepExternalSchema,
}

impl Module {
    /// `chewBBACA.py <이 값>` 으로 그대로 들어가는 CLI 하위 명령 이름.
    pub fn cli_name(self) -> &'static str {
        match self {
            Module::CreateSchema => "CreateSchema",
            Module::AlleleCall => "AlleleCall",
            Module::ExtractCgMLST => "ExtractCgMLST",
            Module::PrepExternalSchema => "PrepExternalSchema",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "CreateSchema" => Some(Module::CreateSchema),
            "AlleleCall" => Some(Module::AlleleCall),
            "ExtractCgMLST" => Some(Module::ExtractCgMLST),
            "PrepExternalSchema" => Some(Module::PrepExternalSchema),
            _ => None,
        }
    }

    /// 결과 폴더가 반드시 있어야 하는가.
    ///
    /// 스키마를 만드는 모듈은 산출물을 앱 저장소에 넣으므로 회수할 것이 없다.
    /// 그런데도 폴더를 필수로 받으면 사용자는 **빈 폴더를 열어보고 실패했다고 생각한다.**
    /// 그래서 선택으로 두고, 지정한 경우에만 실행 로그 사본을 남긴다.
    pub fn requires_output_dir(self) -> bool {
        !self.produces_schema()
    }

    /// 산출물이 앱 저장소의 스키마인가. 그렇다면 `-o` 를 스키마 경로로 겨누고
    /// 끝난 뒤 DB 에 등록해야 한다.
    pub fn produces_schema(self) -> bool {
        matches!(self, Module::CreateSchema | Module::PrepExternalSchema)
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
    /// 결과를 회수할 Windows 폴더. CreateSchema 는 회수할 것이 없어 비어 있을 수 있다.
    pub output_dir: String,
    /// 미지정 시 WSL 내부 `nproc` 값을 사용한다 (§6.4)
    pub cpu: Option<u32>,
    /// 모듈별 파라미터. `flatten` 이라 JSON 은 평평하게 유지된다 —
    /// `{"module":"AlleleCall","outputDir":…,"inputDir":…,"schemaId":…}`
    #[serde(flatten)]
    pub params: ModuleParams,
}

/// 모듈마다 필요한 것이 다르다. 한 구조체에 모두 쌓으면 어떤 조합이 유효한지가
/// 코드 어디에도 적히지 않게 되고, 모듈이 늘 때마다 `Option` 만 늘어난다.
///
/// `module` 을 태그로 쓰므로 이 열거형 자체가 곧 모듈 판정이다 — 별도의 `module`
/// 필드를 두면 둘이 어긋날 수 있다.
/// **`rename_all` 과 `rename_all_fields` 는 서로 다른 것을 가리킨다.**
/// 열거형에서 `rename_all` 은 *variant 이름*을, `rename_all_fields` 는 *필드 이름*을
/// 바꾼다. variant 는 `Module` 및 프런트와 같은 PascalCase(`"CreateSchema"`)여야 하고,
/// 필드는 나머지 타입들처럼 camelCase(`inputDir`)여야 한다.
/// 둘을 헷갈려 `rename_all` 만 붙이면 **모든 모듈의 제출이 깨진다** (실제로 그랬다).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "module",
    rename_all = "PascalCase",
    rename_all_fields = "camelCase"
)]
pub enum ModuleParams {
    CreateSchema {
        /// 어셈블리(FASTA) 폴더
        input_dir: String,
        /// 만들 스키마의 표시 이름
        schema_name: String,
        /// Prodigal training file (Windows 경로). 생략 가능.
        #[serde(default)]
        ptf: Option<String>,
        /// 입력이 이미 CDS 인 경우 `--cds`
        #[serde(default)]
        cds_input: bool,
    },
    AlleleCall {
        input_dir: String,
        /// 사용할 스키마
        schema_id: String,
        /// 일부 loci 만 대상으로 할 때 (`--gl`) 쓸 목록 파일 (Windows 경로)
        #[serde(default)]
        loci_list: Option<String>,
        #[serde(default)]
        cds_input: bool,
    },
    ExtractCgMLST {
        /// AlleleCall 이 만든 `results_alleles.tsv` (Windows 경로).
        /// 이 모듈은 어셈블리 폴더를 쓰지 않는다.
        profiles_file: String,
        /// `--t`. 비우면 chewBBACA 기본값(0.95 / 0.99 / 1)을 모두 계산한다.
        #[serde(default)]
        thresholds: Option<String>,
    },
    PrepExternalSchema {
        /// 들여올 외부 스키마 폴더. loci 마다 FASTA 파일 하나가 들어 있다.
        schema_dir: String,
        /// 앱 저장소에 등록될 표시 이름
        schema_name: String,
        #[serde(default)]
        ptf: Option<String>,
    },
}

impl JobSpec {
    pub fn module(&self) -> Module {
        match self.params {
            ModuleParams::CreateSchema { .. } => Module::CreateSchema,
            ModuleParams::AlleleCall { .. } => Module::AlleleCall,
            ModuleParams::ExtractCgMLST { .. } => Module::ExtractCgMLST,
            ModuleParams::PrepExternalSchema { .. } => Module::PrepExternalSchema,
        }
    }

    /// **폴더**를 입력으로 받는 모듈이면 그 경로. ext4 로 통째로 복사할 대상이다.
    ///
    /// PrepExternalSchema 의 입력은 어셈블리가 아니라 스키마 폴더지만, loci 마다
    /// FASTA 파일 하나씩 수천 개가 들어 있어 9p 위에서 다루면 안 되는 것은 같다.
    pub fn input_dir(&self) -> Option<&str> {
        match &self.params {
            ModuleParams::CreateSchema { input_dir, .. }
            | ModuleParams::AlleleCall { input_dir, .. } => Some(input_dir),
            ModuleParams::PrepExternalSchema { schema_dir, .. } => Some(schema_dir),
            ModuleParams::ExtractCgMLST { .. } => None,
        }
    }

    pub fn cds_input(&self) -> bool {
        match self.params {
            ModuleParams::CreateSchema { cds_input, .. }
            | ModuleParams::AlleleCall { cds_input, .. } => cds_input,
            _ => false,
        }
    }

    /// 스키마를 만드는 모듈의 Prodigal training file.
    pub fn ptf(&self) -> Option<&str> {
        match &self.params {
            ModuleParams::CreateSchema { ptf, .. }
            | ModuleParams::PrepExternalSchema { ptf, .. } => ptf.as_deref(),
            _ => None,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// **프런트가 실제로 보내는 JSON 을 그대로 넣는다.**
    ///
    /// `src/lib/types.ts` 의 `JobSpec` 유니온과 1:1 이어야 하며, 어긋나면
    /// `jobs_submit` 이 "unknown variant" 로 거절한다 — 실제로 그런 회귀가 있었다
    /// (`rename_all` 이 필드가 아니라 variant 를 바꾼다는 것을 놓쳤다).
    /// 이 테스트는 그 조합을 문자열 수준에서 고정한다.
    fn assert_roundtrip(json: &str, expect: Module) {
        let spec: JobSpec = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("프런트 JSON 을 받지 못한다: {e}\n{json}"));
        assert_eq!(spec.module(), expect);

        // 다시 직렬화한 것도 프런트가 보낸 것과 같은 모양이어야 한다.
        // DB 의 args 로 저장됐다가 조정 때 다시 읽히기 때문이다.
        let back = serde_json::to_string(&spec).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&back).unwrap();
        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        for (k, v) in original.as_object().unwrap() {
            assert_eq!(parsed.get(k), Some(v), "필드 `{k}` 가 왕복에서 어긋났다");
        }
    }

    #[test]
    fn create_schema_json_from_the_form() {
        assert_roundtrip(
            r#"{"outputDir":"","cpu":null,"module":"CreateSchema",
                "inputDir":"C:/genomes","schemaName":"내 스키마",
                "ptf":null,"cdsInput":false}"#,
            Module::CreateSchema,
        );
    }

    #[test]
    fn allele_call_json_from_the_form() {
        assert_roundtrip(
            r#"{"outputDir":"C:/out","cpu":8,"module":"AlleleCall",
                "inputDir":"C:/genomes","schemaId":"s-1234abcd",
                "lociList":"C:/cgMLSTschema95.txt","cdsInput":true}"#,
            Module::AlleleCall,
        );
    }

    #[test]
    fn extract_cgmlst_json_from_the_form() {
        assert_roundtrip(
            r#"{"outputDir":"C:/out","cpu":null,"module":"ExtractCgMLST",
                "profilesFile":"C:/results_alleles.tsv","thresholds":"0.95 0.99 1"}"#,
            Module::ExtractCgMLST,
        );
    }

    #[test]
    fn prep_external_schema_json_from_the_form() {
        assert_roundtrip(
            r#"{"outputDir":"","cpu":4,"module":"PrepExternalSchema",
                "schemaDir":"C:/external","schemaName":"Ridom cgMLST","ptf":null}"#,
            Module::PrepExternalSchema,
        );
    }

    #[test]
    fn module_tag_is_pascal_case_like_the_module_enum() {
        // variant 이름이 Module 의 직렬화 형태와 같아야 한다. 프런트는 하나만 안다.
        let spec: JobSpec = serde_json::from_str(
            r#"{"outputDir":"","cpu":null,"module":"CreateSchema",
                "inputDir":"x","schemaName":"n","ptf":null,"cdsInput":false}"#,
        )
        .unwrap();
        let json = serde_json::to_value(&spec).unwrap();
        assert_eq!(json["module"], serde_json::to_value(Module::CreateSchema).unwrap());
    }
}
