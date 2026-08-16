//! MCP over Streamable HTTP — 프레이밍과 인증 (`doc/MCP.md` §2, §6).
//!
//! 서버→클라이언트 알림을 쓰지 않으므로 최소 구현으로 충분하다. `POST /mcp` 에
//! JSON 으로 답하고 `GET /mcp` 에는 405 를 준다(명세가 허용한다). 세션 ID 도
//! 발급하지 않는다 — 발급하면 클라이언트가 그것을 되돌려줘야 하고, 우리가 모르는
//! 세션에 404 를 줘야 하는 상태 관리가 생긴다. 무상태로 두는 편이 안전하다.

use std::io::Read;
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tiny_http::{Header, Request, Response, Server};

use crate::api;
use crate::commands::AppState;
use crate::models::Module;

use super::{docs, tools};

/// 우리가 아는 프로토콜 판본. 클라이언트가 이 중 하나를 요구하면 그대로 돌려준다.
const KNOWN_VERSIONS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];
const LATEST_VERSION: &str = "2025-06-18";

/// 요청 본문 상한. MCP 호출은 작다 — 이보다 크면 우리 쪽 문제가 아니다.
const MAX_BODY: usize = 1024 * 1024;

pub fn serve(app: AppHandle, server: Arc<Server>) {
    for request in server.incoming_requests() {
        let app = app.clone();
        // 요청마다 스레드를 쓴다. waitSeconds 가 붙은 호출이 최대 60초를 잡고 있어도
        // 수락 루프가 막히지 않아야 한다.
        std::thread::spawn(move || handle(&app, request));
    }
}

/// 접속 기록 파일의 상한. 넘으면 지우고 새로 쓴다 — 진단용이라 최근 것만 있으면 된다.
const LOG_MAX_BYTES: u64 = 256 * 1024;

/// 요청 한 줄을 `%LOCALAPPDATA%\ChewieApp\mcp.log` 에 남긴다.
///
/// **연결이 안 될 때 클라이언트가 접속조차 못 한 것인지, 와서 거절당한 것인지를
/// 가릴 방법이 이것밖에 없다.** 실패 증상이 양쪽 다 "도구가 안 보인다" 로 같다.
fn log_request(app: &AppHandle, line: &str) {
    use std::io::Write;

    let path = app.state::<AppState>().paths.root.join("mcp.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > LOG_MAX_BYTES) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{} {}", crate::util::now_iso(), line);
    }
}

fn handle(app: &AppHandle, mut request: Request) {
    let method = request.method().as_str().to_ascii_uppercase();
    let path = request.url().split('?').next().unwrap_or("/").to_string();

    // 헤더는 거절 여부와 무관하게 먼저 찍는다. 토큰 값 자체는 남기지 않는다.
    let accept = find_header(&request, "accept").unwrap_or_else(|| "-".into());
    let origin = find_header(&request, "origin").unwrap_or_else(|| "-".into());
    let has_auth = find_header(&request, "authorization").is_some();
    let agent = find_header(&request, "user-agent").unwrap_or_else(|| "-".into());
    log_request(
        app,
        &format!(
            "{method} {path} auth={has_auth} accept=\"{accept}\" origin={origin} ua=\"{agent}\""
        ),
    );

    // CORS 프리플라이트. 우리는 브라우저를 대상으로 하지 않지만, 거절하더라도
    // 조용히 끝나는 편이 낫다.
    if method == "OPTIONS" {
        let _ = request.respond(Response::empty(204));
        return;
    }

    if path != "/mcp" {
        // 사용자가 브라우저로 포트를 확인해 보는 경우가 있다(가이드가 그렇게 안내한다).
        // 토큰 없이도 "살아 있다" 는 것만 알려주고, 루트는 200 으로 답해 브라우저가
        // 자기 오류 페이지를 대신 그리지 않게 한다.
        let body = "chewBBACA Desktop MCP server\nendpoint: POST /mcp\n";
        let status = if path == "/" { 200 } else { 404 };
        let _ = request.respond(Response::from_string(body).with_status_code(status));
        return;
    }

    match method.as_str() {
        // 서버→클라이언트 스트림을 열지 않는다. 명세가 허용하는 응답이다.
        "GET" => {
            log_request(app, "  → 405 (SSE 스트림은 열지 않는다)");
            let allow = header(&b"Allow"[..], &b"POST"[..]);
            let _ = request.respond(Response::empty(405).with_header(allow));
            return;
        }
        // 세션을 만들지 않으므로 지울 것도 없다.
        "DELETE" => {
            let _ = request.respond(Response::empty(200));
            return;
        }
        "POST" => {}
        _ => {
            let _ = request.respond(Response::empty(405));
            return;
        }
    }

    // ---- 게이트 ----
    //
    // Origin 은 **있을 때만** 검사한다. 데스크톱 MCP 클라이언트는 이 헤더를 보내지
    // 않으므로 필수로 만들면 아무도 붙지 못한다. 이 검사의 목적은 브라우저 페이지가
    // DNS rebinding 으로 로컬 서버를 부르는 것을 막는 것뿐이고, 인증은 토큰이 한다.
    if let Some(origin) = find_header(&request, "origin") {
        let ok = origin.starts_with("http://127.0.0.1")
            || origin.starts_with("http://localhost")
            || origin.starts_with("https://127.0.0.1")
            || origin.starts_with("https://localhost");
        if !ok {
            log_request(app, "  → 403 (허용되지 않은 Origin)");
            let _ =
                request.respond(Response::from_string("forbidden origin").with_status_code(403));
            return;
        }
    }

    let expected = {
        let state = app.state::<AppState>();
        state.settings().mcp.token
    };
    let presented = find_header(&request, "authorization")
        .and_then(|v| {
            let v = v.trim().to_string();
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
                .map(str::to_string)
        })
        .unwrap_or_default();

    if expected.is_empty() || presented.trim() != expected {
        log_request(
            app,
            if presented.is_empty() {
                "  → 401 (Authorization 헤더가 없음)"
            } else {
                "  → 401 (토큰 불일치)"
            },
        );
        let challenge = header(&b"WWW-Authenticate"[..], &b"Bearer"[..]);
        let _ = request.respond(
            Response::from_string("unauthorized")
                .with_status_code(401)
                .with_header(challenge),
        );
        return;
    }

    // ---- 본문 ----
    let mut body = String::new();
    if request
        .as_reader()
        .take(MAX_BODY as u64)
        .read_to_string(&mut body)
        .is_err()
    {
        let _ = respond_json(
            request,
            400,
            &error_body(None, -32700, "본문을 읽을 수 없습니다"),
        );
        return;
    }

    let parsed: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            let _ = respond_json(
                request,
                400,
                &error_body(None, -32700, &format!("JSON 파싱 실패: {e}")),
            );
            return;
        }
    };

    log_request(
        app,
        &format!(
            "  → 요청 메서드: {}",
            match &parsed {
                Value::Array(items) => items
                    .iter()
                    .filter_map(|m| m.get("method").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join(", "),
                other => other
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or("(없음)")
                    .to_string(),
            }
        ),
    );

    let reply = match parsed {
        // 배치는 2025-06-18 에서 빠졌지만, 보내는 클라이언트가 있으면 받아준다.
        Value::Array(items) => {
            let out: Vec<Value> = items.iter().filter_map(|m| dispatch(app, m)).collect();
            if out.is_empty() {
                None
            } else {
                Some(Value::Array(out))
            }
        }
        other => dispatch(app, &other),
    };

    match reply {
        // 알림(id 없음)에는 돌려줄 것이 없다.
        None => {
            let _ = request.respond(Response::empty(202));
        }
        Some(value) => {
            let _ = respond_json(request, 200, &value);
        }
    }
}

// ================================================================ JSON-RPC

/// 메시지 하나를 처리한다. 알림이면 `None`.
fn dispatch(app: &AppHandle, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = msg.get("params").cloned().unwrap_or(Value::Null);

    // 알림은 처리하고 답하지 않는다.
    if id.is_none() {
        return None;
    }

    let result = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools::list(allow_run(app)) })),
        "tools/call" => tools_call(app, &params),
        "resources/list" => Ok(json!({ "resources": resource_list() })),
        "resources/templates/list" => Ok(json!({ "resourceTemplates": resource_templates() })),
        "resources/read" => resources_read(app, &params),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        "" => Err((-32600, "method 가 없습니다".to_string())),
        other => Err((-32601, format!("지원하지 않는 메서드입니다: {other}"))),
    };

    Some(match result {
        Ok(value) => json!({ "jsonrpc": "2.0", "id": id, "result": value }),
        Err((code, message)) => error_body(id, code, &message),
    })
}

fn initialize(params: &Value) -> Value {
    // 클라이언트가 아는 판본을 요구하면 그대로 맞춰준다. 모르는 값이면 우리 최신을
    // 돌려주고, 붙을지 말지는 클라이언트가 정한다 (명세가 정한 절차다).
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let version = if KNOWN_VERSIONS.contains(&requested) {
        requested
    } else {
        LATEST_VERSION
    };

    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {}, "resources": {} },
        "serverInfo": {
            "name": "chewbbaca-desktop",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": "chewBBACA(세균 유전체 cg/wgMLST 분석)를 Windows 에서 돌리는 데스크톱 앱입니다. 표준 순서는 CreateSchema → AlleleCall → ExtractCgMLST 입니다. 실행 도구는 오래 걸리는 작업을 큐에 넣고 jobId 를 돌려주므로, chewie_get_job 으로 완료를 확인하세요. 경로는 모두 Windows 절대 경로여야 하며 네트워크(UNC) 경로는 지원하지 않습니다.",
    })
}

fn tools_call(app: &AppHandle, params: &Value) -> std::result::Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "name 이 필요합니다".to_string()))?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let state = app.state::<AppState>();
    let allow = state.settings().mcp.allow_run;
    let outcome = tools::call(app, state.inner(), name, &args, allow);

    // 실행 실패는 프로토콜 오류가 아니라 **모델이 읽고 고쳐야 하는 결과**다.
    // JSON-RPC 에러로 올리면 클라이언트가 내용을 감추는 경우가 있다.
    Ok(match outcome {
        Ok(text) => json!({ "content": [ { "type": "text", "text": text } ], "isError": false }),
        Err(text) => json!({ "content": [ { "type": "text", "text": text } ], "isError": true }),
    })
}

fn allow_run(app: &AppHandle) -> bool {
    app.state::<AppState>().settings().mcp.allow_run
}

// ================================================================ 리소스

fn resource_list() -> Vec<Value> {
    let mut v = vec![
        json!({
            "uri": "chewie://modules",
            "name": "chewBBACA 모듈 요약",
            "description": "여덟 모듈이 각각 무엇을 하는지, 표준 파이프라인 순서는 무엇인지.",
            "mimeType": "text/markdown",
        }),
        json!({
            "uri": "chewie://schemas",
            "name": "등록된 스키마",
            "description": "앱이 보관 중인 스키마 목록(JSON).",
            "mimeType": "application/json",
        }),
        json!({
            "uri": "chewie://guide",
            "name": "따라해보기 가이드",
            "description": "앱에 동봉된 사용 안내 문서(HTML).",
            "mimeType": "text/html",
        }),
    ];
    for m in docs::ALL_MODULES {
        v.push(json!({
            "uri": format!("chewie://modules/{}", m.cli_name()),
            "name": format!("{} 사용법", m.cli_name()),
            "description": docs::doc(m).summary,
            "mimeType": "text/markdown",
        }));
    }
    v
}

fn resource_templates() -> Vec<Value> {
    vec![
        json!({
            "uriTemplate": "chewie://modules/{module}",
            "name": "모듈 사용법",
            "description": "모듈 하나의 인자·전제조건·주의사항.",
            "mimeType": "text/markdown",
        }),
        json!({
            "uriTemplate": "chewie://jobs/{jobId}/log",
            "name": "작업 로그",
            "description": "작업 하나의 전체 실행 로그. 길 수 있으므로 원인 추적에만 쓴다.",
            "mimeType": "text/plain",
        }),
    ]
}

fn resources_read(app: &AppHandle, params: &Value) -> std::result::Result<Value, (i64, String)> {
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .ok_or((-32602, "uri 가 필요합니다".to_string()))?;

    let state = app.state::<AppState>();

    let (mime, text) = match uri {
        "chewie://modules" => ("text/markdown", docs::render_all()),
        "chewie://guide" => (
            "text/html",
            include_str!("../../guide/guide.html").to_string(),
        ),
        "chewie://schemas" => {
            let list = api::schemas_list(state.inner()).map_err(|e| (-32603, e.to_string()))?;
            (
                "application/json",
                serde_json::to_string_pretty(&list).unwrap_or_else(|_| "[]".into()),
            )
        }
        other => {
            if let Some(name) = other.strip_prefix("chewie://modules/") {
                let m = Module::parse(name)
                    .ok_or((-32602, format!("알 수 없는 모듈입니다: {name}")))?;
                ("text/markdown", docs::render(m))
            } else if let Some(rest) = other.strip_prefix("chewie://jobs/") {
                let job_id = rest.strip_suffix("/log").ok_or((
                    -32602,
                    "작업 리소스는 chewie://jobs/{jobId}/log 형태여야 합니다".to_string(),
                ))?;
                let log =
                    api::jobs_log(state.inner(), job_id).map_err(|e| (-32602, e.to_string()))?;
                ("text/plain", log)
            } else {
                return Err((-32602, format!("알 수 없는 리소스입니다: {other}")));
            }
        }
    };

    Ok(json!({
        "contents": [ { "uri": uri, "mimeType": mime, "text": text } ]
    }))
}

// ================================================================ 잡동사니

fn error_body(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": { "code": code, "message": message },
    })
}

fn header(name: &[u8], value: &[u8]) -> Header {
    Header::from_bytes(name, value).expect("정적 헤더는 항상 유효하다")
}

fn find_header(request: &Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str().to_string())
}

fn respond_json(request: Request, status: u16, body: &Value) -> std::io::Result<()> {
    let text = serde_json::to_string(body).unwrap_or_else(|_| "{}".into());
    let ct = header(&b"Content-Type"[..], &b"application/json"[..]);
    request.respond(
        Response::from_string(text)
            .with_status_code(status)
            .with_header(ct),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_echoes_a_known_protocol_version() {
        let r = initialize(&json!({ "protocolVersion": "2025-03-26" }));
        assert_eq!(r["protocolVersion"], "2025-03-26");
    }

    #[test]
    fn initialize_falls_back_for_an_unknown_version() {
        // 모르는 값을 그대로 돌려주면 지원한다고 거짓말하는 셈이 된다.
        let r = initialize(&json!({ "protocolVersion": "1999-01-01" }));
        assert_eq!(r["protocolVersion"], LATEST_VERSION);
        assert_eq!(r["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn every_listed_resource_uri_is_readable_in_shape() {
        // 목록에 있는데 read 에서 못 알아보는 URI 가 있으면 안 된다.
        for r in resource_list() {
            let uri = r["uri"].as_str().unwrap();
            let known = uri == "chewie://modules"
                || uri == "chewie://guide"
                || uri == "chewie://schemas"
                || Module::parse(uri.trim_start_matches("chewie://modules/")).is_some();
            assert!(known, "read 가 모르는 URI 다: {uri}");
        }
    }

    #[test]
    fn resource_list_covers_all_modules() {
        let n = resource_list().len();
        assert_eq!(n, 3 + docs::ALL_MODULES.len());
    }
}
