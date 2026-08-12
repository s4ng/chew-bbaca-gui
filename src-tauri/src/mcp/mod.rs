//! 로컬 MCP 서버 (`doc/MCP.md`).
//!
//! `commands.rs` 와 **형제**인 표현 계층이다. 여기에 WSL·PGID·`/mnt/c` 같은 개념은
//! 등장하지 않으며, 코어는 `api.rs` 를 통해서만 부른다 (§4.1 의 이식 경계).
//!
//! 서버의 수명은 앱의 수명과 같다. 작업은 앱보다 오래 살지만(§6.3) 서버는 그렇지
//! 않다 — 앱이 꺼져 있으면 클라이언트는 연결 거부를 본다. 의도된 동작이다.

pub mod docs;
mod http;
pub mod tools;

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};
use tiny_http::Server;

use crate::commands::AppState;
use crate::error::{Error, Result};
use crate::settings::McpSettings;

/// 설정한 포트가 막혀 있을 때 위로 훑어볼 개수.
const PORT_SCAN: u16 = 10;

pub struct McpServer {
    running: Mutex<Option<Running>>,
}

struct Running {
    server: Arc<Server>,
    port: u16,
}

/// UI 가 그대로 표시하는 상태.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub running: bool,
    pub enabled: bool,
    pub allow_run: bool,
    /// 실제로 열린 포트. 설정값과 다를 수 있다 (충돌 시 다음 포트를 쓴다).
    pub port: Option<u16>,
    /// 실제로 열려 있는 주소. 꺼져 있으면 `None`.
    pub url: Option<String>,
    pub token: String,

    // ---- 붙여넣기용 ----
    //
    // **클라이언트 등록 화면은 URL·헤더 키·헤더 값을 따로 받는 폼이다.** 설정 파일
    // 문법(TOML)을 통째로 주면 사용자가 거기서 값을 눈으로 뜯어내야 한다. 그래서
    // 칸 단위로 그대로 복사할 수 있는 값을 따로 내려보낸다.
    //
    // 서버가 꺼져 있어도 값은 채운다 — 켜기 전에 미리 등록해 두는 경우가 있다.
    pub connect_url: String,
    pub header_name: String,
    pub header_value: String,
    /// 설정 파일을 쓰는 클라이언트(Codex CLI)용 TOML 조각. 부차적인 수단이다.
    pub client_config: String,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            running: Mutex::new(None),
        }
    }

    pub fn port(&self) -> Option<u16> {
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|r| r.port)
    }

    /// 설정을 읽어 서버를 (다시) 띄운다. 이미 떠 있으면 먼저 내린다.
    ///
    /// 토큰이 비어 있으면 여기서 발급해 저장한다 — 기본값에 박아두면 모든 설치본이
    /// 같은 토큰을 쓰게 되므로, 발급 시점은 첫 기동이어야 한다.
    pub fn start(&self, app: &AppHandle) -> Result<Option<u16>> {
        self.stop();

        let state = app.state::<AppState>();
        let mut settings = state.settings();

        if !settings.mcp.enabled {
            return Ok(None);
        }
        if settings.mcp.token.trim().is_empty() {
            settings.mcp.token = McpSettings::new_token();
            settings.save(&state.db)?;
        }

        let (server, port) = bind(settings.mcp.port)?;
        let server = Arc::new(server);

        write_endpoint_file(&state, port, &settings.mcp.token);

        {
            let app = app.clone();
            let server = Arc::clone(&server);
            std::thread::spawn(move || http::serve(app, server));
        }

        *self.running.lock().unwrap_or_else(|e| e.into_inner()) = Some(Running { server, port });
        Ok(Some(port))
    }

    /// 리스너를 닫는다. `unblock()` 이 수락 루프를 끝내면 그 스레드도 함께 끝난다.
    pub fn stop(&self) {
        if let Some(running) = self
            .running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            running.server.unblock();
        }
    }

    pub fn status(&self, state: &AppState) -> McpStatus {
        let settings = state.settings();
        let port = self.port();

        // 토큰이 아직 없으면(서버가 꺼진 채 처음 열어본 경우) 빈 문자열이 나간다.
        // UI 는 그때 [서버 켜기] 를 안내한다.
        let token = settings.mcp.token.clone();
        let url = port.map(|p| format!("http://127.0.0.1:{p}/mcp"));
        // 실제로 열린 포트가 진실이다. 꺼져 있을 때만 설정값으로 대신한다.
        let effective = port.unwrap_or(settings.mcp.port);

        McpStatus {
            running: port.is_some(),
            enabled: settings.mcp.enabled,
            allow_run: settings.mcp.allow_run,
            connect_url: endpoint_url(effective),
            header_name: HEADER_NAME.to_string(),
            header_value: header_value(&token),
            client_config: client_config(effective, &token),
            port,
            url,
            token,
        }
    }
}

/// 요청한 포트부터 위로 훑는다. 실제로 열린 포트는 호출자가 저장/표시해야 한다.
fn bind(preferred: u16) -> Result<(Server, u16)> {
    let start = if preferred == 0 { 8787 } else { preferred };
    let mut last: Option<String> = None;

    for offset in 0..PORT_SCAN {
        let port = start.saturating_add(offset);
        match Server::http(("127.0.0.1", port)) {
            Ok(server) => return Ok((server, port)),
            Err(e) => last = Some(e.to_string()),
        }
    }

    Err(Error::Other(format!(
        "MCP 서버를 열 수 없습니다. {}부터 {}까지 모두 사용 중입니다{}.\n설정에서 다른 포트를 지정하거나, 그 포트를 쓰는 프로그램을 종료하세요.",
        start,
        start.saturating_add(PORT_SCAN - 1),
        last.map(|e| format!(" ({e})")).unwrap_or_default()
    )))
}

/// 접속 정보를 앱 폴더에 남긴다. UI 없이 손으로 설정할 때 쓰라고 두는 것이며,
/// 이 파일이 없어도 앱은 정상 동작한다.
fn write_endpoint_file(state: &AppState, port: u16, token: &str) {
    let payload = serde_json::json!({
        "url": format!("http://127.0.0.1:{port}/mcp"),
        "port": port,
        "token": token,
    });
    let path = state.paths.root.join("mcp.json");
    let _ = std::fs::write(
        path,
        serde_json::to_string_pretty(&payload).unwrap_or_default(),
    );
}

/// 인증 헤더의 이름. 클라이언트 등록 화면의 [헤더] 키 칸에 그대로 들어간다.
pub const HEADER_NAME: &str = "Authorization";

/// 토큰이 아직 없을 때 칸에 보여줄 문구. 빈 칸을 주면 사용자가 복사해 놓고
/// 왜 안 되는지 몰라 헤맨다.
const NO_TOKEN: &str = "<앱을 켜면 발급됩니다>";

pub fn endpoint_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/mcp")
}

/// [헤더] 값 칸에 넣을 문자열. **`Bearer` 와 토큰 사이의 공백 하나가 규격이다** —
/// 사용자가 손으로 조립하면 여기서 자주 틀린다.
pub fn header_value(token: &str) -> String {
    let token = if token.is_empty() { NO_TOKEN } else { token };
    format!("Bearer {token}")
}

/// 설정 파일을 쓰는 클라이언트(Codex CLI 의 `~/.codex/config.toml`)용 조각.
///
/// ChatGPT 데스크톱 앱처럼 폼으로 받는 클라이언트에는 쓸모가 없다 — 그쪽은
/// `connect_url` / `header_name` / `header_value` 를 칸마다 넣는다.
pub fn client_config(port: u16, token: &str) -> String {
    let token = if token.is_empty() { NO_TOKEN } else { token };
    format!(
        "[mcp_servers.chewie]\nurl = \"{}\"\nhttp_headers = {{ {HEADER_NAME} = \"Bearer {token}\" }}\n",
        endpoint_url(port)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_config_has_the_url_and_the_token() {
        let s = client_config(8787, "deadbeef");
        assert!(s.contains("http://127.0.0.1:8787/mcp"));
        assert!(s.contains("Bearer deadbeef"));
        // TOML 인라인 테이블이라 중괄호가 그대로 남아야 한다.
        assert!(s.contains("http_headers = { Authorization"));
    }

    #[test]
    fn client_config_is_still_readable_before_the_token_exists() {
        let s = client_config(8787, "");
        assert!(s.contains(NO_TOKEN));
    }

    /// 등록 폼에 그대로 넣을 값들. 여기서 만들어 주지 않으면 사용자가 손으로
    /// 조립하다 `Bearer` 뒤 공백 같은 데서 틀린다.
    #[test]
    fn per_field_values_are_ready_to_paste() {
        assert_eq!(endpoint_url(8787), "http://127.0.0.1:8787/mcp");
        assert_eq!(header_value("deadbeef"), "Bearer deadbeef");
        assert_eq!(HEADER_NAME, "Authorization");
    }

    #[test]
    fn the_toml_snippet_agrees_with_the_per_field_values() {
        // 두 수단이 다른 값을 안내하면 사용자는 어느 쪽이 맞는지 알 수 없다.
        let toml = client_config(9001, "abc123");
        assert!(toml.contains(&endpoint_url(9001)));
        assert!(toml.contains(&header_value("abc123")));
        assert!(toml.contains(HEADER_NAME));
    }

    #[test]
    fn bind_scans_upward_when_the_port_is_taken() {
        // 먼저 잡아두고, 같은 포트를 요청하면 다음 포트로 넘어가야 한다.
        let (first, port) = bind(18787).expect("첫 포트를 열지 못했다");
        let (second, next) = bind(port).expect("다음 포트로 넘어가지 못했다");
        assert_eq!(next, port + 1);
        drop(first);
        drop(second);
    }
}
