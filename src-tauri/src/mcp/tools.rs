//! MCP 도구 표면 (`doc/MCP.md` §4).
//!
//! 여기서 하는 일은 **도구 인자 → `JobSpec` 변환과 결과 문자열 만들기**뿐이다.
//! 검증은 하지 않는다 — 그것은 `jobs.rs::submit()` 의 몫이고, 게이트를 두 벌
//! 유지하면 반드시 어긋난다.
//!
//! 모듈마다 도구를 하나씩 둔다. 단일 `chewie_run(module, params)` 보다 모델의
//! 성공률이 높고, `Module` 에 대한 `match` 라서 모듈이 늘면 컴파일이 깨진다.

use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use tauri::AppHandle;

use crate::api;
use crate::commands::AppState;
use crate::models::{JobSpec, JobStatus, Module};

use super::docs;

/// 도구 호출 결과. `Err` 는 MCP 의 `isError: true` 로 나간다 —
/// 프로토콜 오류가 아니라 **모델이 읽고 고쳐야 하는 실패**다.
pub type ToolResult = std::result::Result<String, String>;

/// 로그 꼬리의 상한. 전체 로그는 수 MB 라 컨텍스트를 통째로 태운다.
const MAX_TAIL: usize = 2000;
/// `waitSeconds` 의 상한. 이보다 길면 클라이언트 타임아웃에 걸린다.
const MAX_WAIT: u64 = 60;

// ================================================================ 목록

pub fn list(allow_run: bool) -> Vec<Value> {
    let mut tools = vec![
        tool(
            "chewie_status",
            "chewBBACA 백엔드 상태를 확인한다. 다른 도구를 쓰기 전에, 또는 실행이 실패할 때 원인을 좁히기 위해 먼저 부른다. 배포판 준비 여부, chewBBACA 버전, CPU 개수, 디스크 사용량, 실행 중/대기 중 작업 수를 돌려준다.",
            json!({}),
            &[],
        ),
        tool(
            "chewie_list_schemas",
            "앱이 보관 중인 스키마 목록. AlleleCall·SchemaEvaluator·AlleleCallEvaluator 는 schemaId 를 요구하므로, 그 값이 필요할 때 이 도구로 얻는다.",
            json!({}),
            &[],
        ),
        tool(
            "chewie_list_jobs",
            "최근 작업 이력. 무엇이 돌고 있는지, 지난 실행의 결과 폴더가 어디인지 확인할 때 부른다.",
            json!({ "limit": { "type": "integer", "description": "최대 개수 (기본 20)", "minimum": 1 } }),
            &[],
        ),
        tool(
            "chewie_get_job",
            "작업 하나의 현재 상태·진행률·결과 경로. 실행 도구가 돌려준 jobId 로 완료 여부를 확인할 때 쓴다.",
            json!({ "jobId": { "type": "string" } }),
            &["jobId"],
        ),
        tool(
            "chewie_job_log",
            "작업 로그의 **끝부분**. 실패 원인을 찾을 때 부른다. 전체 로그는 수 MB 가 될 수 있어 꼬리만 돌려준다.",
            json!({
                "jobId": { "type": "string" },
                "tail": { "type": "integer", "description": "돌려받을 줄 수 (기본 200, 최대 2000)", "minimum": 1 }
            }),
            &["jobId"],
        ),
        tool(
            "chewie_inspect",
            "경로를 실행 전에 진단한다. 폴더면 FASTA 파일이 몇 개인지, 파일이면 allelic profile 표인지 loci 목록인지 알려준다. **잘못된 파일을 넣으면 chewBBACA 가 거절하지 않고 한참 헛돌기 때문에**, 사용자가 준 경로로 작업을 제출하기 전에 부르는 것이 좋다.",
            json!({ "path": { "type": "string", "description": "Windows 절대 경로" } }),
            &["path"],
        ),
        tool(
            "chewie_module_help",
            "모듈의 인자·전제조건·주의사항을 읽는다. 어떤 모듈을 쓸지 고를 때, 그리고 실행 도구를 처음 부르기 전에 확인한다. module 을 생략하면 여덟 모듈의 요약을 돌려준다.",
            json!({ "module": { "type": "string", "enum": module_names() } }),
            &[],
        ),
    ];

    if allow_run {
        for m in docs::ALL_MODULES {
            let schema = params_schema(m);
            let props = schema["properties"].clone();
            let required: Vec<String> = schema["required"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let required: Vec<&str> = required.iter().map(String::as_str).collect();
            tools.push(tool(tool_name(m), run_description(m), props, &required));
        }
        tools.push(tool(
            "chewie_cancel",
            "실행 중인 작업을 중단한다. 프로세스 그룹 전체를 종료하므로 BLAST 같은 자식 프로세스도 함께 정리된다.",
            json!({ "jobId": { "type": "string" } }),
            &["jobId"],
        ));
        tools.push(tool(
            "chewie_open_report",
            "완료된 평가 작업(SchemaEvaluator·AlleleCallEvaluator)의 HTML 리포트를 사용자의 기본 브라우저로 연다. 사용자 화면에 창이 뜨므로 요청받았을 때만 부른다.",
            json!({ "jobId": { "type": "string" } }),
            &["jobId"],
        ));
    }

    tools
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
        }
    })
}

fn module_names() -> Vec<&'static str> {
    docs::ALL_MODULES.iter().map(|m| m.cli_name()).collect()
}

// ================================================================ 호출

pub fn call(
    app: &AppHandle,
    state: &AppState,
    name: &str,
    args: &Value,
    allow_run: bool,
) -> ToolResult {
    // 읽기 도구부터. 실행 도구는 allow_run 이 꺼져 있으면 목록에도 없고 여기서도 막는다.
    match name {
        "chewie_status" => return status(state),
        "chewie_list_schemas" => return list_schemas(state),
        "chewie_list_jobs" => return list_jobs(state, args),
        "chewie_get_job" => return get_job(state, args),
        "chewie_job_log" => return job_log(state, args),
        "chewie_inspect" => return inspect(args),
        "chewie_module_help" => return module_help(args),
        _ => {}
    }

    if !allow_run {
        return Err(
            "이 앱의 MCP 설정에서 [작업 실행 허용] 이 꺼져 있어 실행 도구를 쓸 수 없습니다.\n앱의 설정 화면에서 켜달라고 사용자에게 요청하세요. 읽기 도구는 그대로 쓸 수 있습니다."
                .into(),
        );
    }

    match name {
        "chewie_cancel" => cancel(state, args),
        "chewie_open_report" => open_report(app, state, args),
        _ => match module_for_tool(name) {
            Some(module) => submit(state, module, args),
            None => Err(format!("알 수 없는 도구입니다: {name}")),
        },
    }
}

// ---------------------------------------------------------------- 읽기

fn status(state: &AppState) -> ToolResult {
    let backend = api::backend_status(state);
    let disk = api::disk_usage(state);
    let jobs = api::jobs_list(state, 200).unwrap_or_default();
    let running = jobs
        .iter()
        .filter(|j| j.status == JobStatus::Running)
        .count();
    let queued = jobs.iter().filter(|j| j.status == JobStatus::Queued).count();

    ok_json(&json!({
        "ready": backend.ready,
        "chewbbacaVersion": backend.chewbbaca_version,
        "cpuCount": backend.cpu_count,
        "detail": backend.detail,
        "runningJobs": running,
        "queuedJobs": queued,
        "vhdxBytes": disk.vhdx_bytes,
        "appDir": disk.app_dir,
        "note": if backend.ready {
            "실행 준비가 되어 있습니다."
        } else {
            "백엔드가 준비되지 않았습니다. 사용자에게 앱의 온보딩 화면을 확인하도록 안내하세요 — MCP 로는 환경을 구성할 수 없습니다."
        }
    }))
}

fn list_schemas(state: &AppState) -> ToolResult {
    let schemas = api::schemas_list(state).map_err(|e| e.to_string())?;
    if schemas.is_empty() {
        return Ok(
            "등록된 스키마가 없습니다. chewie_create_schema 로 새로 만들거나 chewie_prep_external_schema 로 외부 스키마를 들여오세요."
                .into(),
        );
    }
    ok_json(&json!(schemas))
}

fn list_jobs(state: &AppState, args: &Value) -> ToolResult {
    let limit = args.get("limit").and_then(Value::as_i64).unwrap_or(20);
    let jobs = api::jobs_list(state, limit.clamp(1, 200)).map_err(|e| e.to_string())?;
    ok_json(&json!(jobs))
}

fn get_job(state: &AppState, args: &Value) -> ToolResult {
    let id = str_arg(args, "jobId")?;
    match api::jobs_get(state, id).map_err(|e| e.to_string())? {
        Some(job) => ok_json(&json!(job)),
        None => Err(format!("그런 작업이 없습니다: {id}")),
    }
}

fn job_log(state: &AppState, args: &Value) -> ToolResult {
    let id = str_arg(args, "jobId")?;
    let tail = args
        .get("tail")
        .and_then(Value::as_u64)
        .unwrap_or(200)
        .min(MAX_TAIL as u64) as usize;

    let text = api::jobs_log(state, id).map_err(|e| e.to_string())?;
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(tail);
    let shown = &lines[start..];

    if start > 0 {
        Ok(format!(
            "(앞의 {start}줄 생략, 마지막 {}줄)\n{}",
            shown.len(),
            shown.join("\n")
        ))
    } else if shown.is_empty() {
        Ok("로그가 아직 비어 있습니다.".into())
    } else {
        Ok(shown.join("\n"))
    }
}

fn inspect(args: &Value) -> ToolResult {
    let path = str_arg(args, "path")?.to_string();
    let p = std::path::Path::new(&path);

    if p.is_dir() {
        let info =
            crate::commands::inspect_input_dir(path.clone()).map_err(|e| e.to_string())?;
        return ok_json(&json!({
            "kind": "directory",
            "totalFiles": info.total_files,
            "fastaFiles": info.fasta_files,
            "note": "FASTA 파일 수가 0 이면 어셈블리 폴더가 아닙니다.",
        }));
    }
    if !p.is_file() {
        return Err(format!("경로를 찾을 수 없습니다: {path}"));
    }

    // 파일이면 두 가지로 다 읽어 보고 판단은 호출자에게 맡긴다. 확장자로는
    // allelic profile 표와 loci 목록이 구별되지 않는다.
    let profiles = crate::commands::inspect_profiles_file(path.clone()).ok();
    let loci = crate::commands::inspect_loci_list(path.clone()).ok();

    ok_json(&json!({
        "kind": "file",
        "profileTable": profiles.as_ref().map(|i| json!({
            "looksValid": i.looks_valid,
            "genomes": i.genomes,
            "loci": i.loci,
            "firstColumn": i.first_column,
        })),
        "lociList": loci.as_ref().map(|i| json!({
            "looksValid": i.looks_valid,
            "loci": i.loci,
            "tabbed": i.tabbed,
            "firstLine": i.first_line,
        })),
        "note": "profileTable.looksValid 가 참이면 AlleleCall 결과 표(results_alleles.tsv)이고, lociList.looksValid 가 참이면 --gl 이나 RemoveGenes 에 넣는 loci 목록입니다. 둘 다 거짓이면 잘못된 파일입니다.",
    }))
}

fn module_help(args: &Value) -> ToolResult {
    match args.get("module").and_then(Value::as_str) {
        None => Ok(docs::render_all()),
        Some(name) => match Module::parse(name) {
            Some(m) => Ok(docs::render(m)),
            None => Err(format!(
                "알 수 없는 모듈입니다: {name}\n가능한 값: {}",
                module_names().join(", ")
            )),
        },
    }
}

// ---------------------------------------------------------------- 실행

fn submit(state: &AppState, module: Module, args: &Value) -> ToolResult {
    let spec = build_spec(module, args)?;
    let job_id = api::jobs_submit(state, spec).map_err(|e| e.to_string())?;

    // 실행 슬롯은 1개다. 앞에 뭐가 있으면 이 작업은 큐에서 기다린다 —
    // 이 사실을 적지 않으면 호출자가 "시작되지 않았다" 고 오해한다.
    let ahead = api::jobs_list(state, 200)
        .unwrap_or_default()
        .iter()
        .filter(|j| {
            j.job_id != job_id && matches!(j.status, JobStatus::Running | JobStatus::Queued)
        })
        .count();

    let wait = args.get("waitSeconds").and_then(Value::as_u64).unwrap_or(0);
    let finished = if wait > 0 {
        wait_for(state, &job_id, wait.min(MAX_WAIT))
    } else {
        None
    };

    let mut out = format!(
        "{} 작업을 제출했습니다.\njobId: {}\n",
        module.cli_name(),
        job_id
    );
    match finished {
        Some(job) if job.status.is_terminal() => {
            out.push_str(&format!("상태: {}\n", job.status.as_str()));
            if let Some(path) = job.output_path.as_deref().filter(|p| !p.is_empty()) {
                out.push_str(&format!("결과: {path}\n"));
            }
            if let Some(err) = job.error.as_deref() {
                out.push_str(&format!("오류: {err}\n"));
                out.push_str("chewie_job_log 로 로그를 확인하세요.\n");
            }
        }
        _ => {
            if ahead > 0 {
                out.push_str(&format!(
                    "상태: 대기 중 (앞에 {ahead}건). 실행 슬롯은 하나라 순서대로 돕니다.\n"
                ));
            } else {
                out.push_str("상태: 실행을 시작했습니다.\n");
            }
            out.push_str(
                "이 작업은 오래 걸릴 수 있습니다. chewie_get_job(jobId) 으로 진행률을, 실패하면 chewie_job_log(jobId) 로 원인을 확인하세요.\n",
            );
        }
    }
    Ok(out)
}

/// 짧은 모듈용 대기. 상한을 두는 이유는 클라이언트 쪽 타임아웃이다.
fn wait_for(state: &AppState, job_id: &str, seconds: u64) -> Option<crate::models::Job> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut last = None;
    while Instant::now() < deadline {
        match api::jobs_get(state, job_id) {
            Ok(Some(job)) => {
                let terminal = job.status.is_terminal();
                last = Some(job);
                if terminal {
                    return last;
                }
            }
            Ok(None) => return None,
            Err(_) => return last,
        }
        std::thread::sleep(Duration::from_millis(700));
    }
    last
}

fn cancel(state: &AppState, args: &Value) -> ToolResult {
    let id = str_arg(args, "jobId")?;
    api::jobs_cancel(state, id).map_err(|e| e.to_string())?;
    Ok(format!("작업 {id} 에 중단을 요청했습니다. 프로세스 그룹이 정리될 때까지 몇 초 걸릴 수 있습니다."))
}

fn open_report(app: &AppHandle, state: &AppState, args: &Value) -> ToolResult {
    let id = str_arg(args, "jobId")?;
    let path = api::report_open(app, state, id).map_err(|e| e.to_string())?;
    Ok(format!("사용자의 기본 브라우저로 리포트를 열었습니다: {path}"))
}

// ---------------------------------------------------------------- 인자 → JobSpec

/// 도구 인자를 `JobSpec` JSON 으로 바꿔 serde 에 그대로 넘긴다.
///
/// **스키마에 선언된 키만 통과시킨다.** 모델이 넣은 여분의 키가 flatten 된
/// 열거형에 섞여 들어가면 해석이 어긋날 수 있고, 그때는 조용히 틀린다.
fn build_spec(module: Module, args: &Value) -> std::result::Result<JobSpec, String> {
    let schema = params_schema(module);
    let props = schema["properties"].as_object().cloned().unwrap_or_default();

    let mut o = Map::new();
    for key in props.keys() {
        if key == "waitSeconds" || key == "outputDir" || key == "cpu" {
            continue;
        }
        if let Some(v) = args.get(key) {
            if !v.is_null() {
                o.insert(key.clone(), v.clone());
            }
        }
    }

    // 이 셋은 항상 명시한다 — 빠뜨렸을 때의 serde 동작에 기대지 않는다.
    o.insert("module".into(), json!(module.cli_name()));
    o.insert(
        "outputDir".into(),
        args.get("outputDir")
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or_else(|| json!("")),
    );
    o.insert(
        "cpu".into(),
        args.get("cpu").filter(|v| !v.is_null()).cloned().unwrap_or(Value::Null),
    );

    serde_json::from_value(Value::Object(o))
        .map_err(|e| format!("인자를 해석할 수 없습니다: {e}\nchewie_module_help 로 필요한 인자를 확인하세요."))
}

pub fn tool_name(module: Module) -> &'static str {
    match module {
        Module::CreateSchema => "chewie_create_schema",
        Module::AlleleCall => "chewie_allele_call",
        Module::ExtractCgMLST => "chewie_extract_cgmlst",
        Module::PrepExternalSchema => "chewie_prep_external_schema",
        Module::RemoveGenes => "chewie_remove_genes",
        Module::JoinProfiles => "chewie_join_profiles",
        Module::SchemaEvaluator => "chewie_schema_evaluator",
        Module::AlleleCallEvaluator => "chewie_allele_call_evaluator",
    }
}

fn module_for_tool(name: &str) -> Option<Module> {
    docs::ALL_MODULES
        .iter()
        .copied()
        .find(|m| tool_name(*m) == name)
}

fn run_description(module: Module) -> &'static str {
    // 도구 설명은 요청마다 컨텍스트에 실린다. 한 줄 요약 + 언제 부르는지까지만 적고,
    // 상세는 chewie_module_help 로 미룬다.
    match module {
        Module::CreateSchema => "어셈블리 폴더에서 새 스키마를 만든다(파이프라인 1단계). 시간이 오래 걸리므로 jobId 를 받아 폴링한다.",
        Module::AlleleCall => "어셈블리를 기존 스키마에 대해 allele calling 한다(2단계). schemaId 는 chewie_list_schemas 로 얻는다.",
        Module::ExtractCgMLST => "AlleleCall 결과 표에서 core genome loci 를 추린다. 입력은 폴더가 아니라 results_alleles.tsv 파일 하나다.",
        Module::PrepExternalSchema => "외부 스키마 폴더를 chewBBACA 형식으로 변환해 앱에 등록한다. CreateSchema 를 대신하는 1단계다.",
        Module::RemoveGenes => "프로파일 표에서 지정한 loci 를 제거한다(keepInstead 를 켜면 그것만 남긴다).",
        Module::JoinProfiles => "여러 번 나눠 돌린 프로파일 표를 하나로 합친다. 균주가 겹치지 않는 결과들이어야 한다.",
        Module::SchemaEvaluator => "스키마 품질 리포트(HTML)를 만든다. lociReports 를 켜면 크게 느려진다.",
        Module::AlleleCallEvaluator => "AlleleCall 결과의 품질 리포트(HTML)를 만든다. 결과 폴더에 cds_coordinates.tsv 가 있어야 한다.",
    }
}

/// 모듈별 입력 스키마. `ModuleParams` 의 필드와 이름이 1:1 이어야 한다 —
/// 어긋나면 `build_spec` 이 실패하고, 그것을 아래 테스트가 잡는다.
fn params_schema(module: Module) -> Value {
    let out_dir = json!({ "type": "string", "description": "결과를 회수할 Windows 절대 경로" });
    let cpu = json!({ "type": "integer", "description": "생략하면 WSL 의 nproc 값을 쓴다", "minimum": 1 });
    let wait = json!({ "type": "integer", "description": "완료를 이만큼(초, 최대 60) 기다렸다가 결과를 돌려준다. 오래 걸리는 모듈에는 쓰지 않는다", "minimum": 1 });

    match module {
        Module::CreateSchema => json!({
            "properties": {
                "inputDir": { "type": "string", "description": "어셈블리 FASTA 폴더 (Windows 절대 경로)" },
                "schemaName": { "type": "string", "description": "만들 스키마의 표시 이름" },
                "ptf": { "type": "string", "description": "Prodigal training file 경로 (선택)" },
                "cdsInput": { "type": "boolean", "description": "입력이 이미 CDS 면 true (--cds)" },
                "outputDir": { "type": "string", "description": "생략 가능 — 산출물은 앱 저장소로 간다" },
                "cpu": cpu, "waitSeconds": wait,
            },
            "required": ["inputDir", "schemaName"],
        }),
        Module::AlleleCall => json!({
            "properties": {
                "inputDir": { "type": "string", "description": "어셈블리 FASTA 폴더 (Windows 절대 경로)" },
                "schemaId": { "type": "string", "description": "chewie_list_schemas 의 schemaId" },
                "outputDir": out_dir,
                "lociList": { "type": "string", "description": "일부 loci 만 대상으로 할 때의 목록 파일 (--gl, 선택)" },
                "cdsInput": { "type": "boolean", "description": "입력이 이미 CDS 면 true (--cds)" },
                "cpu": cpu, "waitSeconds": wait,
            },
            "required": ["inputDir", "schemaId", "outputDir"],
        }),
        Module::ExtractCgMLST => json!({
            "properties": {
                "profilesFile": { "type": "string", "description": "AlleleCall 이 만든 results_alleles.tsv 경로" },
                "outputDir": out_dir,
                "thresholds": { "type": "string", "description": "예: \"0.95 0.99 1\". 비우면 기본값 전부" },
                "waitSeconds": wait,
            },
            "required": ["profilesFile", "outputDir"],
        }),
        Module::PrepExternalSchema => json!({
            "properties": {
                "schemaDir": { "type": "string", "description": "loci FASTA 가 든 외부 스키마 폴더" },
                "schemaName": { "type": "string", "description": "앱에 등록할 표시 이름" },
                "ptf": { "type": "string", "description": "Prodigal training file 경로 (선택)" },
                "outputDir": { "type": "string", "description": "생략 가능 — 산출물은 앱 저장소로 간다" },
                "cpu": cpu, "waitSeconds": wait,
            },
            "required": ["schemaDir", "schemaName"],
        }),
        Module::RemoveGenes => json!({
            "properties": {
                "profilesFile": { "type": "string", "description": "걸러낼 프로파일 표" },
                "genesList": { "type": "string", "description": "대상 loci 목록 파일 (한 줄에 하나)" },
                "outputDir": out_dir,
                "keepInstead": { "type": "boolean", "description": "켜면 목록에 있는 것만 남긴다 (--inverse)" },
                "waitSeconds": wait,
            },
            "required": ["profilesFile", "genesList", "outputDir"],
        }),
        Module::JoinProfiles => json!({
            "properties": {
                "profilesFiles": {
                    "type": "array", "items": { "type": "string" }, "minItems": 2,
                    "description": "합칠 프로파일 표들. 균주가 겹치면 안 된다",
                },
                "outputDir": out_dir,
                "commonOnly": { "type": "boolean", "description": "공통 loci 만으로 합친다 (--common)" },
                "waitSeconds": wait,
            },
            "required": ["profilesFiles", "outputDir"],
        }),
        Module::SchemaEvaluator => json!({
            "properties": {
                "schemaId": { "type": "string", "description": "chewie_list_schemas 의 schemaId" },
                "outputDir": out_dir,
                "lociReports": { "type": "boolean", "description": "loci 마다 상세 페이지를 만든다. 크게 느려진다" },
                "cpu": cpu, "waitSeconds": wait,
            },
            "required": ["schemaId", "outputDir"],
        }),
        Module::AlleleCallEvaluator => json!({
            "properties": {
                "resultsDir": { "type": "string", "description": "results_alleles.tsv 가 든 AlleleCall 결과 폴더" },
                "schemaId": { "type": "string", "description": "그 결과를 만들 때 쓴 스키마" },
                "outputDir": out_dir,
                "cpu": cpu, "waitSeconds": wait,
            },
            "required": ["resultsDir", "schemaId", "outputDir"],
        }),
    }
}

// ---------------------------------------------------------------- 잡동사니

fn str_arg<'a>(args: &'a Value, key: &str) -> std::result::Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("인자 `{key}` 가 필요합니다."))
}

fn ok_json(v: &Value) -> ToolResult {
    serde_json::to_string_pretty(v).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 스키마의 필수 인자만 채운 최소 호출이 `JobSpec` 으로 해석되어야 한다.
    ///
    /// **이것이 드리프트를 막는 진짜 방어선이다.** `ModuleParams` 의 필드 이름을
    /// 바꾸면 Rust 도 컴파일되고 스키마도 그럴듯하지만 실제 호출만 조용히 깨진다 —
    /// `models.rs` 의 왕복 테스트와 같은 성격의 사고가 이미 한 번 있었다.
    fn sample_value(name: &str, spec: &Value) -> Value {
        match spec["type"].as_str() {
            Some("boolean") => json!(false),
            Some("integer") => json!(4),
            Some("array") => json!([
                format!("C:/{name}/a.tsv"),
                format!("C:/{name}/b.tsv")
            ]),
            _ => json!(format!("C:/{name}")),
        }
    }

    #[test]
    fn every_run_tool_builds_a_valid_job_spec_from_its_required_args() {
        for m in docs::ALL_MODULES {
            let schema = params_schema(m);
            let props = schema["properties"].as_object().unwrap();
            let mut args = Map::new();
            for key in schema["required"].as_array().unwrap() {
                let key = key.as_str().unwrap();
                args.insert(key.to_string(), sample_value(key, &props[key]));
            }
            let spec = build_spec(m, &Value::Object(args))
                .unwrap_or_else(|e| panic!("{}: {e}", m.cli_name()));
            assert_eq!(spec.module(), m, "{} 의 태그가 어긋났다", m.cli_name());
        }
    }

    #[test]
    fn optional_args_also_round_trip() {
        // 선택 인자까지 모두 채운 호출도 해석되어야 한다.
        for m in docs::ALL_MODULES {
            let schema = params_schema(m);
            let props = schema["properties"].as_object().unwrap();
            let mut args = Map::new();
            for (key, spec) in props {
                args.insert(key.clone(), sample_value(key, spec));
            }
            let spec = build_spec(m, &Value::Object(args))
                .unwrap_or_else(|e| panic!("{}: {e}", m.cli_name()));
            assert_eq!(spec.module(), m);
        }
    }

    #[test]
    fn unknown_keys_are_dropped_before_serde_sees_them() {
        let args = json!({
            "inputDir": "C:/genomes",
            "schemaName": "내 스키마",
            "waitSeconds": 10,
            "somethingTheModelInvented": "x",
        });
        let spec = build_spec(Module::CreateSchema, &args).unwrap();
        assert_eq!(spec.module(), Module::CreateSchema);
        assert_eq!(spec.output_dir, "");
        assert_eq!(spec.cpu, None);
    }

    #[test]
    fn the_module_tag_matches_the_serde_representation() {
        // build_spec 은 cli_name() 을 태그로 쓴다. 둘이 갈리면 모든 실행이 깨진다.
        for m in docs::ALL_MODULES {
            assert_eq!(serde_json::to_value(m).unwrap(), json!(m.cli_name()));
        }
    }

    #[test]
    fn run_tools_disappear_when_running_is_not_allowed() {
        let read_only = list(false);
        assert!(!read_only.is_empty());
        for t in &read_only {
            let name = t["name"].as_str().unwrap();
            assert!(
                module_for_tool(name).is_none() && name != "chewie_cancel",
                "읽기 전용인데 실행 도구가 남아 있다: {name}"
            );
        }
        assert_eq!(list(true).len(), read_only.len() + 10);
    }

    #[test]
    fn every_tool_has_a_description_and_an_object_schema() {
        for t in list(true) {
            let name = t["name"].as_str().unwrap();
            assert!(
                t["description"].as_str().is_some_and(|d| d.len() > 20),
                "{name} 설명이 부실하다"
            );
            assert_eq!(t["inputSchema"]["type"], "object", "{name}");
        }
    }
}
