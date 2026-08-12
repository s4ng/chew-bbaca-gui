# MCP 서버 설계

**작성:** 2026-08-12 · **상태:** v0.3.0 에 구현 완료. 실측 결과는 §9 에 있다.

앱이 켜져 있는 동안 로컬에서 MCP 서버를 돌려, 외부 MCP 클라이언트가 이 앱의 기능을
읽고 실행할 수 있게 한다. 설계 근거는 [`ARCHITECTURE.md`](../ARCHITECTURE.md),
지켜야 할 불변조건은 [`CLAUDE.md`](../CLAUDE.md) 에 있다. 이 문서는 그 둘 위에
MCP 계층을 얹을 때의 결정과 그 이유만 담는다.

---

## 1. 무엇을 만드는가

MCP 클라이언트가 이 앱에 붙어서 할 수 있는 일 세 가지.

- **읽는다** — 백엔드 상태, 스키마 목록, 작업 목록·진행률·로그
- **실행한다** — 여덟 모듈 실행(작업 제출), 취소, 리포트 열기
- **배운다** — 각 모듈의 인자·전제·함정을 MCP resource 로 읽는다

### 왜 서버가 앱 **안**에 있어야 하는가

작업을 실제로 돌릴 수 있는 것은 이 앱 프로세스뿐이다. `JobManager` 가 실행 슬롯과
`setsid`·PGID·SQLite 를 소유하므로(§6.2), 별도 프로세스가 DB 에 행만 넣어봤자
그것을 집어 실행할 주체가 없다. 그래서 MCP 서버는 앱에 내장한다.

그 결과로 따라오는 성질 하나 — **MCP 서버의 수명은 앱의 수명과 같다.** 작업은 앱보다
오래 살지만(§6.3) 서버는 그렇지 않다. 앱이 꺼져 있으면 클라이언트는 연결 거부를 본다.
이것은 결함이 아니라 의도된 동작이며, 설정 화면에 그렇게 적는다.

---

## 2. 토폴로지 — 앱 내장 HTTP 하나

```
MCP 클라이언트 ──Streamable HTTP──▶ 127.0.0.1:8787/mcp  [ChewieApp]
                (베어러 토큰)                              └ src-tauri/src/mcp/
```

**stdio 브리지 실행 파일은 만들지 않는다.** 초안에는 `chewie-mcp.exe`(stdin ↔ HTTP
펌프)를 두는 안이 있었으나, 대상 클라이언트를 확인한 결과 불필요해졌다.

### 대상 클라이언트 — ChatGPT Desktop 기준

ChatGPT Desktop 은 Codex 와 MCP 설정을 공유하며 **stdio 와 Streamable HTTP 를 모두**
받고, **localhost URL 을 허용한다**(공식 문서 예시가 `http://localhost:3000/mcp`).
인증은 `bearer_token_env_var` 또는 `http_headers`(정적 값)로 헤더를 넣는다.

사용자가 `~/.codex/config.toml` 에 넣을 것은 이게 전부다. 설정 화면의
[클라이언트 설정 복사] 버튼이 이 세 줄을 만들어 준다.

```toml
[mcp_servers.chewie]
url = "http://127.0.0.1:8787/mcp"
http_headers = { Authorization = "Bearer <앱이 발급한 토큰>" }
```

> **ChatGPT 웹/모바일의 "커넥터(개발자 모드)"는 다른 물건이다.** 그쪽은 OpenAI 서버가
> 직접 접속하는 **공개 HTTPS** 엔드포인트만 받으므로 localhost 가 원천적으로 불가능하고
> (터널이 필요하다), Plus/Pro 는 읽기 전용이라는 보고가 있다. 우리 실행 도구 8개는
> 쓰기 성격이므로 **지원 대상은 데스크톱 앱 경로로 한정한다.**

stdio 를 택하지 않은 이유는 하나 더 있다. stdio 는 클라이언트가 서버 프로세스를
띄우는 모델이라, ChatGPT 가 켜질 때마다 우리 GUI 앱이 함께 뜨게 된다. 연결 거부가
그보다 정직하다.

### 프로토콜 구현

직접 구현한다. 서버→클라이언트 알림이 필요 없으면 Streamable HTTP 의 최소 준수
구현은 `POST /mcp` 에 JSON 으로 답하고 `GET` 에 405 를 주면 된다(스펙상 허용).
필요한 메서드는 `initialize` · `notifications/initialized` · `tools/list` ·
`tools/call` · `resources/list` · `resources/read` · `ping` 뿐이다.

`rmcp`(공식 Rust SDK)를 쓰면 SSE·진행 알림까지 얻지만 tokio + axum 이 통째로 들어온다.
이 저장소는 `ureq`·`std::thread` 로 동기 스타일을 유지하고 있고, 릴리스 프로필이
`lto` + `opt-level = "s"` 라 대가가 작지 않다. **P0 에서 실제 클라이언트로 검증하고,
막히면 그때 갈아탄다**(§9 참조) — 읽기 도구밖에 없는 시점이 전환 비용이 가장 싸다.

HTTP 서버는 `tiny_http` 하나면 된다(동기, 스레드).

---

## 3. 코드 배치 — 이식 경계는 그대로

```
src-tauri/src/
  commands.rs      Tauri IPC (기존)
  api.rs           ← 신설. commands.rs 의 알맹이를 여기로 내렸다
  mcp/
    mod.rs         서버 기동/종료, 포트 선점, mcp.json 기록, 설정 조각 생성
    http.rs        HTTP + JSON-RPC 프레이밍, Origin/토큰 검사, 리소스
    tools.rs       도구 표 → api:: 호출, 인자 → JobSpec 변환
    docs.rs        모듈 사용법 단일 진실 원천 (§5)
```

`mcp/` 는 `commands.rs` 의 **형제**다. 둘 다 표현 계층이고, 그 아래 `api.rs` 가
`&AppState`(필요 시 `&AppHandle`)를 받는 평범한 함수 모음이다. `commands.rs` 에는
`#[tauri::command]` 껍데기만 남는다.

이렇게 나누는 이유는 **게이트를 한 벌만 유지하기 위해서다.** `jobs.rs::submit()` 의
loci 목록 검사, `cds_coordinates.tsv` 전제 검사, `validate_host_path()` 가 MCP 경로에도
그대로 걸려야 한다. **MCP 도구가 `submit()` 을 우회해 `runner` 를 직접 부르는 설계는
금지다.**

`mcp/` 안에도 WSL·PGID·`/mnt/c` 는 등장하지 않는다(§4.1).

---

## 4. 도구 표면

모듈마다 도구를 하나씩 둔다. 단일 `chewie_run(module, params)` + `oneOf` 스키마보다
모델의 성공률이 높고, `ModuleParams` variant 와 1:1 이라 새 모듈이 늘 때 빠뜨리면
컴파일이 깨지게 만들 수 있다.

| 도구 | 하는 일 | 등급 |
| --- | --- | --- |
| `chewie_status` | 배포판·chewBBACA 버전·디스크·큐 상태 | 읽기 |
| `chewie_list_schemas` | 스키마 목록 (loci 수 포함) | 읽기 |
| `chewie_list_jobs` / `chewie_get_job` | 이력·상태·진행률·결과 경로 | 읽기 |
| `chewie_job_log(jobId, tail=200)` | 로그 **꼬리만**. 전체는 수 MB다 | 읽기 |
| `chewie_inspect(path)` | `inspect_input_dir`·`inspect_profiles_file`·`inspect_loci_list` 를 경로 종류로 분기한 하나 | 읽기 |
| `chewie_module_help(module?)` | 인자·전제·함정 (§5) | 읽기 |
| `chewie_create_schema` … `chewie_allele_call_evaluator` (8개) | 작업 제출 → `jobId` 즉시 반환 | 실행 |
| `chewie_cancel(jobId)` | `kill -TERM -{PGID}` 경로 (§6.2) | 실행 |
| `chewie_open_report(jobId)` | 기본 브라우저로 리포트를 연다 | 실행 |

### 노출하지 않는 것

`schemas_delete` · `env_unregister` · `env_provision` · `env_install_wsl` ·
`env_reboot_to_firmware` · `disk_compact` · `settings_set`.

CLAUDE.md 의 "되돌릴 수 없는 조작은 UI 에서 확인을 받은 뒤 호출한다"를 MCP 로 번역하면
**확인을 받을 UI 가 없는 채널에는 아예 주지 않는다**가 된다. 배포판 제거와 재부팅을
도구로 여는 것은 논외다.

### 긴 작업

40분짜리 작업을 도구 호출 하나로 붙들고 있을 수 없다.

- 제출은 **즉시** `jobId` 와 "진행률은 `chewie_get_job` 으로 확인" 안내를 돌려준다.
- 짧은 모듈용으로 `waitSeconds`(상한 60)만 선택 인자로 둔다.
- 실행 슬롯은 1개다. GUI 작업이 도는 중이면 큐에 들어가므로, 결과 텍스트에
  **"대기 중(앞에 N건)"을 반드시 적는다.** 적지 않으면 모델이 실패로 오해한다.

---

## 5. 사용법 노출 — 단일 진실 원천

`mcp/docs.rs` 의 `doc(module) -> ModuleDoc { summary, inputs, outputs, prerequisites,
gotchas }` 가 그것이다. **`Module` 에 대한 `match`** 라서 새 variant 를 더하면 여기서
컴파일이 깨지고, 문서 없이 모듈이 느는 일이 구조적으로 막힌다.

> 설계 초안은 이 표를 `models.rs` 에 두려 했다. `mcp/docs.rs` 로 옮긴 이유는
> `models.rs` 가 값 타입만 담는 파일이기 때문이다 — 컴파일 타임 강제는 그대로다.

여기 들어갈 내용은 이미 저장소에 흩어져 있다 — 평가 두 모듈이 `-o` 가 이미 있으면
거부한다는 것, AlleleCallEvaluator 의 `cds_coordinates.tsv` 전제, `--loci-reports` 가
loci 3,127 에서 3초 → 39초가 된다는 것, JoinProfiles 는 균주가 겹치지 않는 결과를
합쳐야 한다는 것. [`NEXT-SESSION.md`](NEXT-SESSION.md) §4 의 함정 목록이 출처다.

이 표를 세 곳이 함께 쓴다.

- MCP 도구 `chewie_module_help`
- MCP resource `chewie://modules/{Module}`
- (선택) `NewJobPage.tsx` 의 폼 도움말

resource 는 셋 더 노출한다 — `chewie://guide`(따라해보기 HTML), `chewie://schemas`,
`chewie://jobs/{id}/log`. 클라이언트가 로그를 첨부 파일처럼 다룰 수 있다.

---

## 6. 안전

| 항목 | 결정 |
| --- | --- |
| 바인드 | `127.0.0.1` 전용. `0.0.0.0` 은 어떤 설정으로도 열지 않는다 |
| 토큰 | 앱이 발급해 DB 에 보관하는 베어러 토큰. 모든 요청에서 검사 |
| `Origin` | **있을 때만** 검사한다 (아래 주의) |
| 포트 | 기본 8787, 점유 시 +1..+9 탐색. **실제 값**을 `mcp.json` 에 기록하고 설정 화면에 표시 |
| 접속 정보 | `%LOCALAPPDATA%\ChewieApp\mcp.json` 에 `{port, token}`. 설정 화면에 [재발급] |
| 접속 기록 | `%LOCALAPPDATA%\ChewieApp\mcp.log` 에 요청 한 줄씩(메서드·auth 유무·Accept·Origin·UA). **토큰 값은 남기지 않는다.** 256KB 넘으면 지우고 새로 쓴다 |
| 경로 | 기존 `validate_host_path()` 가 그대로 걸린다. 다만 `outputDir` 가 기존 폴더를 덮어쓸 수 있으므로, 비어 있지 않은 폴더면 결과 텍스트에 경고를 넣는다 |
| 기본값 | 설정에 `mcp.enabled`(기본 true), `mcp.port`. 끄면 리스너를 만들지 않는다 |

> **`Origin` 검사는 "없으면 통과, 있는데 허용 목록 밖이면 403" 이어야 한다.**
> 데스크톱 MCP 클라이언트는 `Origin` 을 보내지 않는다. 헤더를 필수로 하면
> ChatGPT Desktop 이 아예 붙지 못한다. 이 검사의 목적은 브라우저 페이지가
> DNS rebinding 으로 로컬 MCP 를 호출하는 것을 막는 것뿐이다. 인증은 토큰이 한다.

### 승인 게이트는 두 겹이다

Codex/ChatGPT 쪽에도 도구 승인 모드(`auto` / `prompt` / `writes` / `approve`)가 있다.
우리 쪽은 그 아래층이고, **자동 승인 + 통째로 끄는 스위치**(`mcp.allowRun`) 하나만 둔다.
꺼면 실행 도구가 `tools/list` 에서 아예 사라지고 호출도 거절된다 — 모델에게 부를 수
없는 도구를 보여주지 않는 편이 낫다.

앱 창에 [승인]/[거부]를 띄우는 안은 채택하지 않았다. 앱을 보고 있지 않으면 작업이
그대로 멈추기 때문이다.

---

## 7. 수명주기와 설정

`lib.rs::setup()` 에서 `app.manage(AppState)` **다음에** `mcp::start(app.handle(), &settings)`.
종료는 `RunEvent::Exit` 에서 리스너를 닫는다(`tiny_http::Server::unblock`).

설정 화면에 넣을 것.

- 상태 한 줄 — `실행 중 · http://127.0.0.1:8787/mcp`
- **[클라이언트에 넣을 값]** — URL · 헤더 키 · 헤더 값을 칸마다 두고 각각 [복사].
  등록 화면이 폼이라 **복사도 칸 단위여야 한다** — TOML 을 통째로 주면 사용자가
  거기서 값을 눈으로 뜯어내야 한다. §2 의 TOML 은 설정 파일을 쓰는 클라이언트용으로
  접어서 남긴다
- [연결 방법 보기] — 그림이 든 HTML 안내를 브라우저로 연다 (`guide/mcp.html`).
  `Work` 모드 함정이 여기 맨 앞에 적혀 있다
- [토큰 재발급], MCP 사용 여부 토글, 작업 실행 허용 토글
- "MCP 서버는 앱이 켜져 있는 동안에만 동작합니다" 안내

---

## 8. 테스트

- `mcp/docs.rs` — 모든 `Module` 이 비어 있지 않은 문서를 갖는다
- `mcp/tools.rs` — **각 모듈 도구의 JSON Schema 로 만든 최소 인자 조합이
  `serde_json::from_value::<JobSpec>()` 를 통과한다.** `models.rs` 의 기존 왕복
  테스트와 같은 성격이고, 이것이 드리프트를 막는 진짜 방어선이다
  (Rust 도 컴파일되고 `tsc` 도 통과하는데 문자열 표현만 어긋나는 사고가 이미 있었다)
- `mcp/http.rs` — 토큰 없음/틀림 → 401, 외부 `Origin` → 403, `Origin` 없음 → 통과,
  `initialize` 왕복

---

## 9. 구현 상태와 실측 (2026-08-12, v0.3.0)

P0~P2 를 모두 구현했다. 아래는 개발 실행에 HTTP 로 직접 붙어 확인한 것이다.

| 항목 | 결과 |
| --- | --- |
| Rust 테스트 | **86/86 통과** (MCP 관련 15개 추가) |
| `initialize` | `protocolVersion` 협상, `serverInfo.version = 0.3.0` |
| `notifications/initialized` | 202, 본문 없음 |
| `tools/list` | **17개** (읽기 7 + 실행 8 + cancel + open_report) |
| `tools/call chewie_status` | 배포판 준비됨, chewBBACA 3.5.4, CPU 12, vhdx 2.18GB |
| `resources/read chewie://modules/*` | 모듈 사용법 반환 |
| 토큰 없음 / 틀린 토큰 | 401 |
| `Origin: https://evil.example` | **403** |
| `Origin: http://localhost:1420` | 200 |
| `GET /mcp` | 405 (`Allow: POST`) |
| 외부 인터페이스(LAN IP) 접속 | 연결 자체가 안 됨 — 127.0.0.1 바인드 확인 |
| 입력 게이트 | UNC 경로·필수 인자 누락이 `isError` 로 한국어 안내와 함께 돌아옴 |
| **실제 실행** | `chewie_create_schema`(완성 게놈 32개, `waitSeconds=60`) → **49초에 completed**, loci 3,130 스키마가 앱에 등록됨 |
| 앱 종료 | 포트가 즉시 닫힘 |

### ChatGPT Desktop 실접속 — 통과 (2026-08-12)

**P0 의 완료 조건을 달성했다.** 서버 기록에 실제 클라이언트가 찍혔다.

```
POST /mcp auth=true accept="text/event-stream, application/json" ua="codex-mcp-client/0.147.0-alpha.6.6"
  → initialize → notifications/initialized → tools/list → resources/list → resources/templates/list
```

요청 24건, 거절 0건. **손으로 짠 최소 Streamable HTTP 구현이 실제 클라이언트를
통과한다** — `rmcp` 로 갈아탈 이유가 없어졌다. MCP 공식 Inspector
(`@modelcontextprotocol/inspector --cli`)로도 도구 17개 목록과 `tools/call` 실행을 확인했다.

여기까지 오는 데 걸린 함정 셋을 남긴다.

1. **ChatGPT 대화창을 `Work` 모드로 바꿔야 도구가 보인다.** 등록이 완벽해도 `Chat`
   모드에서는 모델에게 도구가 노출되지 않는다. 증상이 "그런 도구가 없다" 라서
   설정 문제로 오해하기 딱 좋다 — **실제로 여기서 가장 오래 막혔다.**
2. **토큰은 [헤더] 에 넣는다.** 폼의 [기본 token 환경 변수] 칸은 이름 그대로
   *환경 변수의 이름*을 받는 자리다(빈 칸의 흐린 글씨가 `MCP_BEARER_TOKEN` 이다).
   토큰 값을 그대로 넣으면 인증 헤더가 실리지 않아 401 이 된다.
   → §10 의 미검증 가정 1번은 이것으로 해소됐다. **정적 헤더 방식이 통한다.**
3. **포트가 조용히 밀린다.** 앱이 둘 켜져 있으면 나중 것이 8788 로 가는데 클라이언트는
   8787 에 고정돼 있다. 아래 §11 참조.

`Origin` 은 이 클라이언트가 보내지 않았다(`origin=-`). "있을 때만 검사" 로 짠 것이 맞았다.

---

## 10. 남은 미검증 가정

1. ~~`http_headers` 에 `Authorization` 을 정적 값으로~~ ✅ **통한다** (§9).
2. ~~ChatGPT Desktop 의 플랜 제한~~ — **플랜 문제가 아니었다.** `Work` 모드 전환이
   원인이었다(§9). 무료 플랜에서도 등록·연결·도구 사용이 됐다.
3. **여러 클라이언트가 동시에 붙었을 때.** 무상태 서버라 문제가 없어야 하지만
   돌려본 적은 없다. 실행 슬롯은 어차피 하나이므로 최악이라도 큐가 길어질 뿐이다.
4. **`waitSeconds` 로 60초를 잡고 있을 때 클라이언트가 먼저 끊는지.** 짧은 모듈에서만
   쓰라고 적어 두었지만 실제 한계는 재보지 않았다.

---

## 11. 알려진 문제 — 포트가 조용히 밀린다

설정한 포트가 사용 중이면 `bind()` 가 다음 번호로 넘어간다(최대 +9). 원래는
"어떻게든 뜨게 한다" 는 뜻이었는데, **MCP 클라이언트는 URL 을 고정해 두므로 이 동작이
곧 연결 끊김이다.** 앱을 두 개 켜면 바로 재현된다.

지금은 설정 화면의 [상태] 에 실제 주소를 보여주는 것이 전부라 사용자가 알아채기 어렵다.
고칠 방향 셋 중 하나를 골라야 한다.

- 밀렸을 때 설정 화면에 **경고 배너**를 띄운다 (가장 작은 변경)
- 아예 밀지 않고 실패시킨 뒤 "그 포트를 쓰는 프로그램을 끄거나 포트를 바꾸세요" 로 안내
- 이미 우리 앱이 그 포트를 쓰고 있는 경우와 남이 쓰는 경우를 구분해 다르게 안내

접속 기록(`%LOCALAPPDATA%\ChewieApp\mcp.log`)이 있어 진단 자체는 이제 빠르다 —
기록이 비어 있으면 클라이언트가 닿지 못한 것이고, 그 첫 후보가 이 포트 문제다.

> 참고로, 스키마 이름에 한글을 쓰면 백엔드 경로가 `MCP_xx_xx-6c7e04f8` 처럼 된다
> (`util::slugify` 가 비ASCII 를 `x` 로 바꾼다). 표시 이름은 그대로 보존되므로
> 동작에는 문제가 없다. MCP 와 무관한 기존 동작이다.

---

## 참고

- [Model Context Protocol | ChatGPT Learn](https://learn.chatgpt.com/docs/extend/mcp)
- [Model Context Protocol | Codex](https://developers.openai.com/codex/mcp)
- [MCP 명세 — Streamable HTTP transport](https://modelcontextprotocol.io/specification)
- [`rmcp` (Rust MCP SDK)](https://github.com/modelcontextprotocol/rust-sdk)
