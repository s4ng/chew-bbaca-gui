//! 표현 계층 아래의 공용 진입점 (`doc/MCP.md` §3).
//!
//! `commands.rs`(Tauri IPC)와 `mcp/`(로컬 MCP 서버)가 **같은 함수를 통해** 코어를
//! 부른다. 게이트를 한 벌만 유지하기 위한 것이다 — 입력 검증은 `jobs.rs::submit()`
//! 안에 있고, 표현 계층 어느 쪽도 그것을 우회해 `runner` 를 직접 부르지 않는다.
//!
//! 여기 있는 함수는 Tauri 타입(`State`)을 받지 않는다. 그래야 MCP 요청 스레드처럼
//! Tauri command 컨텍스트 밖에서도 호출할 수 있다.

use std::path::{Path, PathBuf};

use tauri::AppHandle;

use crate::commands::{AppState, DiskUsage};
use crate::env::EnvReport;
use crate::error::{Error, Result};
use crate::fasta::GenomeScan;
use crate::models::{Job, JobSpec, Module, SchemaInfo};
use crate::runner::BackendStatus;
use crate::training_store::{TrainingCreated, TrainingFile};

// ================================================================ 환경

pub fn env_probe(state: &AppState) -> Result<EnvReport> {
    crate::env::probe(&state.settings().distro)
}

pub fn backend_status(state: &AppState) -> BackendStatus {
    state.manager.runner().status()
}

pub fn disk_usage(state: &AppState) -> DiskUsage {
    DiskUsage {
        vhdx_bytes: state.provisioner().vhdx_size(),
        app_dir: state.paths.root.to_string_lossy().to_string(),
    }
}

// ================================================================ 작업

pub fn jobs_submit(state: &AppState, spec: JobSpec) -> Result<String> {
    // 다음 실행의 기본값으로 기억해 둔다.
    let mut settings = state.settings();
    settings.last_output_dir = Some(spec.output_dir.clone());
    let _ = settings.save(&state.db);

    state.manager.submit(spec)
}

pub fn jobs_list(state: &AppState, limit: i64) -> Result<Vec<Job>> {
    state.manager.list(limit)
}

pub fn jobs_get(state: &AppState, job_id: &str) -> Result<Option<Job>> {
    state.manager.get(job_id)
}

pub fn jobs_cancel(state: &AppState, job_id: &str) -> Result<()> {
    state.manager.cancel(job_id)
}

pub fn jobs_log(state: &AppState, job_id: &str) -> Result<String> {
    state.manager.read_log(job_id)
}

// ================================================================ 스키마

pub fn schemas_list(state: &AppState) -> Result<Vec<SchemaInfo>> {
    state.schemas().list()
}

// ================================================================ training file

pub fn training_list(state: &AppState) -> Result<Vec<TrainingFile>> {
    state.training().list()
}

/// 게놈 폴더를 훑어 학습 후보를 추린다. 파일을 만들지 않는다.
pub fn training_scan(state: &AppState, genome_dir: &Path) -> Result<GenomeScan> {
    state.training().scan(genome_dir)
}

/// 폴더에서 게놈 하나를 골라 학습시키고 저장소에 넣는다.
///
/// `genome_file` 은 UI 가 사용자의 선택을 넘기는 자리다. MCP 는 생략해 앱이
/// 고르게 한다 — **게이트와 선별 규칙은 어느 쪽이든 여기 한 벌뿐이다.**
pub fn training_create(
    state: &AppState,
    name: &str,
    genome_dir: &Path,
    genome_file: Option<&Path>,
) -> Result<TrainingCreated> {
    state.training().create(name, genome_dir, genome_file)
}

pub fn training_delete(state: &AppState, name: &str) -> Result<()> {
    state.training().delete(name)
}

// ================================================================ 리포트

/// 평가 리포트 HTML 을 기본 브라우저로 연다.
///
/// 앱 웹뷰에 띄우지 않는 이유는 CSP 와 asset 프로토콜을 열어야 하기 때문만이
/// 아니다 — 리포트는 확대·검색·인쇄가 되는 브라우저에서 보는 편이 낫다.
///
/// **여는 것까지 Rust 가 한다.** 프런트의 `openPath` 는 스코프가 비어 있어
/// 어떤 경로도 통과하지 못한다 (`commands::guide_open` 의 주석 참조).
pub fn report_open(app: &AppHandle, state: &AppState, job_id: &str) -> Result<String> {
    use tauri_plugin_opener::OpenerExt;

    let job = state
        .manager
        .get(job_id)?
        .ok_or_else(|| Error::JobNotFound(job_id.to_string()))?;
    let dir = job.output_path.as_deref().unwrap_or_default();
    if dir.is_empty() {
        return Err(Error::Other(
            "결과가 아직 회수되지 않아 리포트를 찾을 수 없습니다".into(),
        ));
    }

    let path = find_report(Path::new(dir), job.module).ok_or_else(|| {
        Error::Other(format!(
            "결과 폴더에서 리포트 HTML 을 찾지 못했습니다.\n폴더를 직접 열어 확인하세요: {dir}"
        ))
    })?;

    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| {
            Error::Other(format!(
                "리포트를 열지 못했습니다: {e}\n파일은 여기 있습니다: {}",
                path.display()
            ))
        })?;
    Ok(path.to_string_lossy().to_string())
}

/// 회수된 결과 폴더에서 리포트 HTML 을 찾는다.
///
/// 파일 이름은 3.5.4 를 직접 돌려 확인했다 (2026-08-11). 그래도 이름만 믿지 않고
/// `*_report.html` 로 한 번 더 훑는다 — 판올림으로 이름이 바뀌어도 열리도록.
fn find_report(dir: &Path, module: Module) -> Option<PathBuf> {
    let known = match module {
        Module::SchemaEvaluator => "schema_report.html",
        Module::AlleleCallEvaluator => "allelecall_report.html",
        _ => return None,
    };
    let direct = dir.join(known);
    if direct.is_file() {
        return Some(direct);
    }
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        let p = e.path();
        let name = p.file_name()?.to_string_lossy().to_ascii_lowercase();
        (name.ends_with("_report.html") && p.is_file()).then_some(p)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 리포트 파일 이름은 3.5.4 를 직접 돌려 확인한 값이다 (2026-08-11).
    /// 여기서 굳혀 두지 않으면 [리포트 열기] 가 조용히 아무것도 못 찾게 된다.
    #[test]
    fn finds_each_module_report_by_its_measured_name() {
        for (module, name) in [
            (Module::SchemaEvaluator, "schema_report.html"),
            (Module::AlleleCallEvaluator, "allelecall_report.html"),
        ] {
            let dir = std::env::temp_dir().join(format!("chewie-report-{name}"));
            std::fs::create_dir_all(&dir).unwrap();
            // 리포트 옆에는 같은 확장자가 아닌 큰 부산물들이 함께 놓인다.
            std::fs::write(dir.join("report_bundle.js"), "x").unwrap();
            std::fs::write(dir.join(name), "<html>").unwrap();

            let found = find_report(&dir, module).expect("리포트를 찾지 못했다");
            assert_eq!(found.file_name().unwrap(), name);
        }
    }

    #[test]
    fn falls_back_to_any_report_html_when_the_name_changed() {
        // 판올림으로 이름이 바뀌어도 열리기는 해야 한다.
        let dir = std::env::temp_dir().join("chewie-report-renamed");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("something_report.html"), "<html>").unwrap();

        let found = find_report(&dir, Module::SchemaEvaluator).unwrap();
        assert_eq!(found.file_name().unwrap(), "something_report.html");
    }

    #[test]
    fn other_modules_have_no_report() {
        // 평가 모듈이 아니면 결과 폴더에 리포트 비슷한 것이 있어도 찾지 않는다.
        let dir = std::env::temp_dir().join("chewie-report-notmine");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("schema_report.html"), "<html>").unwrap();

        assert!(find_report(&dir, Module::AlleleCall).is_none());
    }
}
