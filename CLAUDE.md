# CLAUDE.md

이 저장소에서 코드를 작업할 때 지켜야 할 것들. `ARCHITECTURE.md` 가 기준 문서이고,
이 파일은 그 문서를 코드에 적용할 때 반복적으로 걸리는 지점만 추린 것이다.

> **작업을 이어받는 경우 [`doc/NEXT-SESSION.md`](doc/NEXT-SESSION.md) 를 먼저 읽는다.**
> 어디까지 검증됐고 다음에 무엇을 어디서부터 손대야 하는지가 거기 있다.

## 이 프로젝트가 하는 일

chewBBACA(Linux 전용 CLI)를 Windows 데스크톱 GUI 로 감싼다. 앱은
**Windows GUI 프로세스와 WSL2 전용 배포판(`chewie-env`) 사이의 다리**이며,
복잡도 대부분이 그 경계 — 경로 변환, 프로세스 제어, 파일 이동 — 에 몰려 있다.

## 명령

```powershell
npm install                    # 최초 1회
npm run tauri:dev              # 개발 실행
npm run typecheck              # 프론트 타입 검사 (tsc --noEmit)
npm run build                  # 타입 검사 + 프론트 번들
npm run tauri:build            # NSIS 인스톨러 (perUser)
cargo test --manifest-path src-tauri/Cargo.toml
node scripts/generate-icons.mjs   # 아이콘 재생성
./rootfs/build.sh 3.5.4           # rootfs 이미지 (Linux/WSL 에서)
```

**MSVC 빌드 도구(VS 2022 Build Tools, C++ 워크로드)가 없으면 Rust 는 링크 단계에서
실패한다.** `link.exe failed` 가 보이면 코드 문제가 아니라 환경 문제다. Git for Windows 의
`/usr/bin/link.exe` 가 PATH 에서 먼저 잡히는 경우도 같은 증상을 낸다.

## 절대 깨뜨리면 안 되는 것

### 1. 이식 경계

`src-tauri/src/runner/` 위쪽은 플랫폼 중립이다. `wslpath`, `wsl --import`, PGID,
`/mnt/c` 같은 개념이 `jobs.rs`·`commands.rs`·프론트엔드로 새어나가면 안 된다.
경계를 넘는 값은 **불투명한 문자열**로만 다룬다 (`work_dir`, `backend_path`).

예외는 `env/` 와 `routes/Onboarding.tsx` 다. 온보딩은 본질적으로 Windows/WSL 절차이며
macOS 확장 시 통째로 교체될 화면이다.

### 2. 사용자 환경 불가침

- 기존 WSL 배포판을 읽는 것은 되지만 **수정·삭제는 절대 금지**다. 목록은 표시 전용.
- `.wslconfig` 는 전역 설정이다. 읽지도 쓰지도 않는다. 메모리 제한이 필요하면 안내만 한다.
- `wsl --install` 에서 `--no-distribution` 을 빼지 마라. 빼면 Ubuntu 가 함께 설치된다.
- 앱이 소유하는 것은 `chewie-env` 배포판과 `%LOCALAPPDATA%\ChewieApp` 뿐이다.

### 3. 파일시스템 규칙

`/mnt/c` 접근은 9p 경유로 ext4 대비 5~20배 느리다. chewBBACA 는 수백 개 FASTA 를 읽고
loci FASTA 수천 개를 쓴다.

1. 입력을 `~/work/{job_id}/input` 으로 복사한다.
2. 실행은 WSL 내부 경로에서만 한다.
3. 필요한 결과만 Windows 로 회수한다.

스키마는 `~/schemas/` 에 상주하며 앱이 소유한다. AlleleCall 이 신규 allele 을 계속
추가하므로 Windows 측에 두면 오버헤드가 실행마다 누적된다.

### 4. 프로세스 제어

`wsl.exe` 자식을 kill 해도 내부 Linux 프로세스는 살아남는다. BLAST 가 CPU 를 물고
남으면 사용자 시스템이 마비된다.

- 실행은 `setsid --wait` 로 새 프로세스 그룹에서 한다.
- PGID 는 stdout 표식(`__CHEWIE_PGID__`)으로 즉시 올려보내고 **받자마자 SQLite 에 기록**한다.
- 취소는 실행 중인 `wsl.exe` 를 죽이는 것이 아니라, 별도 프로세스로
  `kill -TERM -{PGID}` → (유예) → `kill -KILL -{PGID}` 를 보낸다.

### 5. 프로세스 기동 세부사항

새 자식 프로세스를 만들 때는 반드시 `win::command()` 를 거친다.

| 항목 | 값 | 빠뜨리면 |
| --- | --- | --- |
| `CREATE_NO_WINDOW` (0x08000000) | 프로세스 생성 플래그 | 검은 콘솔 창이 반복해서 깜빡인다 |
| `WSL_UTF8=1` | 환경변수 | `wsl --status` 등이 UTF-16LE 를 뱉어 파싱이 깨진다 |
| `PYTHONUNBUFFERED=1` | WSL 내부 export | 출력이 버퍼링되어 실시간 진행률이 죽는다 |
| `--cpu` | WSL 내부 `nproc` | Windows 논리 코어 수와 다를 수 있다 |

### 6. 온보딩 순서

**실행을 먼저 시도하고, 실패했을 때만 게이트를 내려간다.** 정상 환경 사용자에게는
아무것도 묻지 않는다.

```
wsl -d chewie-env -- true
  ├─ 성공 → 앱 진입
  └─ 실패 → ① HypervisorPresent → ② wsl --status → ③ rootfs 다운로드/import
```

게이트 ①이 ②보다 먼저인 것은 타협 대상이 아니다. Windows 기능 활성화와 재부팅을
시킨 **뒤에야** `0x80370102` 로 실패하는 경로를 피하기 위한 순서다.

`VirtualizationFirmwareEnabled` 를 단독 판정에 쓰지 마라. 하이퍼바이저가 이미 실행 중이면
`False` 를 돌려주어 정상 기기를 오진한다. `HypervisorPresent == false` 일 때만 보조로 본다.

### 7. 작업은 앱보다 오래 산다

상태를 구조체 필드가 아니라 SQLite 에 둔다. 앱을 닫아도 WSL 안의 작업은 계속 돌고,
다시 켰을 때 그 사실을 알아낼 유일한 근거가 `status=running` 과 `pgid` 다.
로그도 마찬가지 — **파일이 진실이고 Tauri 이벤트는 사본**이다.

## 코드 관례

- **주석은 한국어로**, "무엇을" 이 아니라 **"왜"** 를 쓴다. 코드가 이미 말하는 것을
  반복하지 않는다. 특히 우회 코드에는 우회하는 이유를 남긴다.
- 에러 메시지는 사용자에게 그대로 보이므로 한국어로, **다음에 뭘 해야 하는지**까지 쓴다.
- `Error` 의 `kind` 는 UI 분기용 안정 식별자다. 문자열을 바꾸면 프론트가 조용히 깨진다
  (`elevation-denied` → 수동 명령 안내 폴백처럼 실제로 분기하고 있다).
- 사용자 경로를 셸로 넘길 때는 예외 없이 `util::sh_quote()` 를 거친다. 한글·공백·괄호가
  들어 있는 경로가 기본이라고 가정한다.
- `src/lib/types.ts` 는 Rust serde 표현과 1:1 이다. Rust 구조체를 고치면 **반드시 같이**
  고친다. 어긋나면 런타임에 `undefined` 로 조용히 흐른다.
- 컴포넌트에서 `invoke()` 를 직접 부르지 않는다. `src/lib/ipc.ts` 만 통한다.
- 되돌릴 수 없는 조작(배포판 제거, 스키마 삭제, 재부팅)은 UI 에서 확인을 받은 뒤 호출한다.

## 현재 미완성

- rootfs 배포 URL/체크섬이 비어 있다 (`settings.rs` 의 `TODO(release)`).
  값이 없으면 온보딩 ③ 은 자동 다운로드 대신 안내만 띄운다.
- 진행률 파싱(`runner/progress.rs`)은 휴리스틱이다. 실제 로그로 교정해야 한다.
- v0.2 범위(ExtractCgMLST, 리포트 내장 뷰어, 결과 뷰어 모드)는 미착수.
- 복구한 고아 작업은 stdout 을 재연결할 수 없어 완료 여부만 폴링한다.
