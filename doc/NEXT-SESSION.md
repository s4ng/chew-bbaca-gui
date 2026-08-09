# 다음 세션 인계 노트

**작성:** 2026-08-09 · **이전 세션 범위:** `ARCHITECTURE.md` 기반 기본 구조 구현 (v0.1 골격)

이 문서는 "지금 어디까지 됐고, 다음에 뭘 어디서부터 손대면 되는가" 만 담는다.
설계 근거는 [`ARCHITECTURE.md`](../ARCHITECTURE.md), 지켜야 할 불변조건은
[`CLAUDE.md`](../CLAUDE.md) 에 있다. 둘을 먼저 읽고 이 문서를 본다.

---

## 1. 지금 상태

### 검증된 것 (직접 실행해서 확인함)

| 항목 | 명령 | 결과 |
| --- | --- | --- |
| 프론트 타입 | `npm run typecheck` | 통과 |
| 프론트 번들 | `npx vite build` | 통과 (171KB / gzip 56KB) |
| Rust 빌드 | `cargo build --manifest-path src-tauri/Cargo.toml` | 통과 (경고 6, 전부 dead-code) |
| Rust 테스트 | `cargo test --manifest-path src-tauri/Cargo.toml` | **23/23 통과** |
| 앱 기동 | `npm run tauri:dev` | 창 생성 확인 (`MainWindowTitle: chewBBACA Desktop`) |
| DB 초기화 | — | `%LOCALAPPDATA%\ChewieApp\app.db` 에 `jobs`/`schemas`/`settings` 생성 확인 (WAL) |

### 검증되지 **않은** 것 — 여기가 다음 세션의 본 게임

**실제 chewBBACA 를 한 번도 돌려보지 않았다.** `WslRunner` 의 스크립트 조립·PGID 회수·
스트리밍·스테이징은 전부 "컴파일되고 논리적으로 맞아 보이는" 상태이지 실측된 상태가 아니다.
`chewie-env` 배포판이 아직 존재하지 않기 때문이다.

구체적으로 다음이 미검증이다.

- `setsid --wait bash -c 'echo "__CHEWIE_PGID__ $$"; eval "$CHEWIE_CMD"'` 가
  실제로 PGID 를 올바르게 내보내는지, `kill -TERM -{PGID}` 로 BLAST 까지 죽는지
- `wslpath -a` 에 한글/공백/OneDrive 경로를 넘겼을 때의 실제 동작
- `bash -lc` 에서 micromamba 활성화가 되어 `chewBBACA.py` 가 PATH 에 잡히는지
- `/mnt/c` vs ext4 실행 시간 차이 (§5.2 의 전제 자체)
- `runner/progress.rs` 의 단계 키워드가 실제 출력과 맞는지 (현재는 순수 추측)

---

## 2. 개발 환경 전제 (이 PC 기준)

- Node 24 / npm 8, Rust 1.97 (`x86_64-pc-windows-msvc`)
- **VS 2022 Build Tools 에 C++ 워크로드를 이번 세션에서 추가했다.** (MSVC 14.34 + Windows SDK)
  - 원래 MSBuild 만 설치되어 있어 `link.exe failed` 로 Rust 링크가 전부 실패했다.
  - `winget install ... --override "--add ...VCTools"` 는 **이미 설치된 패키지의 업그레이드
    경로로 빠져 실패한다(exit 1)**. 다음처럼 기존 설치를 *수정*해야 성공한다.
    ```powershell
    & "C:\Program Files (x86)\Microsoft Visual Studio\Installer\setup.exe" modify `
      --installPath "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools" `
      --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --quiet --norestart
    ```
- WSL2 는 설치되어 있고 사용자의 **`Ubuntu` 배포판이 존재한다. 절대 건드리지 말 것.**
- `chewie-env` 배포판은 아직 없다 → 앱을 켜면 온보딩 ③(배포판 게이트)에서 멈춘다.
- `%LOCALAPPDATA%\ChewieApp\` 은 스모크 테스트로 이미 생성되어 있다 (정상, 앱이 재사용한다).
- git 저장소로 초기화되어 있다 (`main` 브랜치, 초기 커밋 `dad4469`). 원격은 아직 없다.
  `.gitattributes` 가 `rootfs/*.sh` 를 LF 로 고정한다 — **이 설정을 풀면 Windows 체크아웃에서
  CRLF 로 바뀌어 rootfs 이미지 안의 `/etc/profile.d/chewie.sh` 가 깨진다.**

---

## 3. 다음 작업 (우선순위 순)

### ① rootfs 를 만들어 실제로 한 번 완주시킨다 — **최우선**

나머지 모든 검증이 여기에 걸려 있다. GUI 를 더 만들기 전에 이것부터 한다.

```bash
# Linux 또는 WSL(Ubuntu) 안에서, Docker 필요
./rootfs/build.sh 3.5.4          # → dist-rootfs/chewie-rootfs-3.5.4.tar.gz + .sha256
```

그다음 둘 중 하나로 등록한다.

- **앱을 통해서:** 산출물을 어딘가에 올리고 `설정 → rootfs 배포 정보` 에 URL/SHA256 입력
  → 온보딩 ③ 의 [내려받고 설치] 버튼. (다운로드 경로까지 함께 검증되므로 이쪽을 권장)
- **수동으로:** `wsl --import chewie-env "%LOCALAPPDATA%\ChewieApp\wsl" <tar.gz> --version 2`
  (다운로드 경로는 건너뛰고 Runner 만 빨리 보고 싶을 때)

등록 후 확인 순서:

1. `wsl -d chewie-env -- bash -lc 'chewBBACA.py --version'` — micromamba 활성화 검증
2. 앱에서 CreateSchema 를 작은 데이터셋으로 실행 → 로그가 **실시간으로** 흐르는지
   (버퍼링되면 `PYTHONUNBUFFERED` 또는 `\r` 처리 문제다)
3. 실행 중 [취소] → `wsl -d chewie-env -- ps aux | grep blast` 로 **잔존 프로세스 0 확인**
4. 완료 후 `스키마` 화면에 등록되는지, `loci 수`/`.trn` 이 잡히는지

**착수 지점:** `rootfs/Dockerfile`, `src-tauri/src/runner/wsl.rs` 의 `spawn_and_stream()`

> Dockerfile 은 아직 한 번도 `docker build` 되지 않았다. micromamba 이미지의
> `MAMBA_ROOT_PREFIX` 기본값이 `/opt/conda` 가 맞는지부터 확인해야 한다
> (`rootfs/profile.d-chewie.sh` 가 이 값을 가정하고 있다).

### ② `/mnt/c` vs ext4 실측 (§5.2 의 전제)

같은 데이터셋으로 두 번 돌려 시간을 잰다. 지금 구조는 "입력을 복사하는 비용보다
9p 오버헤드가 훨씬 크다" 는 가정 위에 서 있다. 만약 차이가 작다면 스테이징 단계
(`WslRunner::stage_input`)를 선택적으로 만들 여지가 생긴다.

### ③ 진행률 파싱 교정

①에서 얻은 **실제 로그 파일**(`%LOCALAPPDATA%\ChewieApp\logs\{job_id}.log`)을 근거로
`src-tauri/src/runner/progress.rs` 의 `STAGES` 테이블을 고친다. 현재 키워드는 추측이며,
테스트(`progress::tests`)도 그 추측 위에 세워져 있으므로 함께 갱신해야 한다.

### ④ 조정(reconciliation) 실경로 확인

작업 실행 중 앱을 강제 종료 → 재실행 → "이전 작업이 실행 중입니다 [복구/종료]" 배너가
뜨는지. `JobManager::reconcile()` 과 `watch_adopted()` 가 이 시나리오에서만 동작한다.

### ⑤ 그다음 (v0.2 범위)

- `ExtractCgMLST` 추가 — `models::Module` 에 variant 추가 → `runner/cli.rs::build_argv`
  → `NewJobPage.tsx` 폼. 이 세 곳이 모듈 추가의 전부다.
- 리포트 내장 뷰어 (SchemaEvaluator HTML) 및 결과 뷰어 모드 (§4.5, §7.7)
- 배포: NSIS 인스톨러 생성 확인(`npm run tauri:build`), GitHub Releases, 업데이터 서명 키

---

## 4. 손대기 전에 알아야 할 함정

- **`sink(...)` 호출부** — `EventSink` 는 `Arc<dyn Fn>` 이고 autoderef 로 호출된다.
  시그니처를 바꿀 일이 있으면 호출부 전체가 함께 깨진다.
- **`Emitter::emit` 은 페이로드에 `Clone` 을 요구한다.** 새 이벤트 구조체를 만들 때
  `Serialize` 만 붙이면 컴파일 에러가 난다 (이번 세션에서 실제로 걸렸다).
- **`Command::args([...])` 의 배열 원소 타입을 섞지 말 것.** `&String`/`Cow<str>` 를
  `&str` 과 함께 넣으면 컴파일되지 않는다. `.as_str()` 로 통일한다.
- **rusqlite `pragma_update` 로 `journal_mode` 를 설정하면 실패한다** (값을 행으로 반환).
  `execute_batch` 를 써야 한다 — `db.rs::open` 참조.
- **`src/lib/types.ts` 와 Rust serde 표현은 수동 동기화다.** 어긋나면 런타임에
  `undefined` 로 조용히 흐른다. Rust 구조체를 고치면 반드시 같이 고친다.
- 개발 빌드 바이너리(`target/debug/chewie-app.exe`)를 단독 실행하면 빈 창이 뜬다.
  `devUrl`(localhost:1420)을 보기 때문이다. 항상 `npm run tauri:dev` 로 띄운다.

---

## 5. 하지 말 것

- 사용자의 `Ubuntu` 배포판 수정/삭제, `.wslconfig` 접근
- `wsl --install` 에서 `--no-distribution` 제거
- `runner/` 밖으로 WSL 개념(`wslpath`, PGID, `/mnt/c`) 노출
- `/mnt/c` 위에서 chewBBACA 실행 (측정 목적의 ②는 예외)
- 실측 없이 `progress.rs` 의 숫자를 "그럴듯하게" 조정하는 것 — 그러면 진짜 교정이 늦어진다
