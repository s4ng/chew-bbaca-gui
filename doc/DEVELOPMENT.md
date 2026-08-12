# 개발 안내

이 문서는 **저장소를 받아 직접 빌드하는 사람**을 위한 것이다. 앱을 쓰기만 한다면
[`README.md`](../README.md) 로 충분하다.

- 설계의 기준 문서는 [`ARCHITECTURE.md`](../ARCHITECTURE.md)
- 코드를 고칠 때 지켜야 할 것은 [`CLAUDE.md`](../CLAUDE.md)
- 진행 상황과 다음 작업은 [`NEXT-SESSION.md`](NEXT-SESSION.md)
- MCP 서버 설계는 [`MCP.md`](MCP.md)

---

## 왜 이런 구조인가

chewBBACA 는 Windows 에서 네이티브로 실행할 수 없다 (Bioconda 가 Linux/macOS 만 지원).
그래서 이 앱은 **Windows GUI 프로세스와 Linux 실행 환경 사이의 다리**다.

```
React (WebView2)  →  Rust / Tauri 2  →  ChewieRunner  →  WSL2 전용 배포판 chewie-env
                                          ↑ 이식 경계        micromamba + chewBBACA 3.5.4
```

핵심 결정 세 가지:

| | |
| --- | --- |
| **전용 WSL 배포판** | 사용자의 기존 배포판·`.wslconfig` 를 건드리지 않는다. 제거는 `wsl --unregister` 한 줄. |
| **모든 I/O 를 ext4 에서** | `/mnt/c` 는 9p 경유라 ext4 대비 5~20배 느리다. 입력을 WSL 내부로 복사한 뒤 실행한다. |
| **상태는 SQLite 에** | 40분 넘게 도는 작업이 있다. 앱을 닫아도 작업은 계속 돌고, 다시 켜면 조정(reconciliation)한다. |

---

## 개발 환경 준비

### 1. 필수 도구

| 도구 | 버전 | 비고 |
| --- | --- | --- |
| Node.js | 18+ | 프론트엔드 빌드 |
| Rust | 1.77+ | `rustup` 권장 |
| **MSVC 빌드 도구** | VS 2022 Build Tools | **필수** — 없으면 Rust 링크 단계에서 실패한다 |
| WebView2 Runtime | — | Windows 11 기본 포함 |

MSVC 빌드 도구가 없으면 `cargo build` 가 다음처럼 실패한다.

```
error: linking with `link.exe` failed: exit code: 1
note: in the Visual Studio installer, ensure the "C++ build tools" workload is selected
```

설치:

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools `
  --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

`rusqlite` 가 SQLite 를 번들 컴파일하므로 C 컴파일러도 이 워크로드에 포함되어야 한다.

> **빌드는 PowerShell 에서 돌린다.** Git Bash 의 PATH 는 `link` 를 coreutils 의
> `/usr/bin/link` 로 잡아 MSVC 링커를 가린다. 증상이
> `could not compile <crate> (build script)` 로만 나와 코드 문제로 오해하기 쉽다.

### 2. 실행

```powershell
npm install
npm run tauri:dev      # 개발 모드 (프론트 HMR + Rust 자동 재빌드)
```

**개발 실행에는 동봉 rootfs 가 없다.** Tauri 는 리소스를 번들 단계에서만 복사하므로
온보딩의 [설치] 버튼이 사라진다. 정상이다 — 설정의 [rootfs 이미지] 칸에
직접 빌드한 tar.gz 의 절대 경로를 넣으면 그 파일을 검증해 등록한다.

### 3. 빌드

```powershell
npm run tauri:build        # dist + NSIS 인스톨러 (perUser). dist-rootfs/ 의 tar.gz 를 동봉한다
npm run tauri:build:slim   # rootfs 없이 GUI 만 (동봉 파일이 없을 때)
```

### 4. 검증

```powershell
npm run typecheck                              # 프론트엔드 타입 검사
cargo test --manifest-path src-tauri/Cargo.toml
```

---

## 저장소 구조

```
├── ARCHITECTURE.md          설계 문서 (이 저장소의 기준 문서)
├── CLAUDE.md                코드 작업 시 지켜야 할 규칙
├── doc/
│   ├── DEVELOPMENT.md       이 문서
│   ├── MCP.md               MCP 서버 설계와 실측
│   └── NEXT-SESSION.md      진행 상황 · 다음 작업 · 함정 목록
├── scripts/
│   ├── check-env.bat        사용자 환경 사전 점검 (단독 실행 가능)
│   └── generate-icons.mjs   앱 아이콘 생성
├── rootfs/                  chewie-env 이미지 빌드 (빌드 타임에만 Docker 사용)
│   ├── Dockerfile
│   └── build.sh
├── src/                     React + TypeScript
│   ├── lib/                 IPC 래퍼 · 타입 · 포맷터
│   └── routes/              온보딩 · 작업 · 스키마 · 설정
└── src-tauri/               Rust / Tauri 2
    ├── guide/               앱에 동봉되는 안내 문서(HTML)와 화면 그림
    └── src/
        ├── runner/          ★ 이식 경계 — WSL 특화 로직은 여기까지만
        ├── env/             환경 검사 게이트 + 배포판 프로비저닝
        ├── mcp/             로컬 MCP 서버 (commands.rs 의 형제)
        ├── api.rs           표현 계층 아래의 공용 진입점
        ├── jobs.rs          작업 수명주기 (큐 · 취소 · 조정)
        ├── db.rs            SQLite
        └── commands.rs      Tauri IPC 표면
```

---

## rootfs 빌드

배포용 이미지는 CI 또는 Linux/WSL 환경에서 만든다. **사용자 PC 에는 Docker 가 필요 없다.**

```bash
./rootfs/build.sh 3.5.4
# → dist-rootfs/chewie-rootfs-3.5.4.tar.gz (+ .sha256)
```

만들어진 tar.gz 는 인스톨러에 동봉된다. **파일명은 네 곳에 흩어져 있다** —
`rootfs/build.sh`, `src-tauri/tauri.bundle.json`, `settings.rs` 의 `file_name`,
그리고 같은 파일의 `sha256`. 하나만 고치면 인스톨러는 정상 생성되고 사용자 기기에서
"찾을 수 없음" 또는 체크섬 불일치로 처음 실행할 때 깨진다.

---

## 기능 범위

| 릴리스 | 범위 |
| --- | --- |
| **v0.1** | 온보딩/환경 구성, `CreateSchema`·`AlleleCall`·`ExtractCgMLST`, 실시간 로그·진행 표시·취소, 스키마 내보내기, 따라해보기 가이드 |
| **v0.2** | `PrepExternalSchema`, 스키마 불러오기, `RemoveGenes`·`JoinProfiles`, `SchemaEvaluator`·`AlleleCallEvaluator` |
| **v0.3** | 로컬 MCP 서버 — MCP 클라이언트에서 대화로 앱을 부린다 |
| 이후 | `DownloadSchema`, `UniprotFinder` 주석, 결과 뷰어 모드 |

평가 리포트의 **내장 뷰어는 만들지 않기로 했다** — 리포트는 확대·검색·인쇄가 되는
기본 브라우저에서 여는 편이 낫다. [작업 상세] 의 [리포트 열기] 가 회수된 HTML 을
브라우저로 넘긴다.

**범위 제외:** macOS/Linux 지원, Docker 실행, 클러스터/HPC 연동, Chewie-NS 쓰기 계열 기능.

---

## 검증 상태

튜토리얼 데이터(*S. agalactiae* 완성 게놈 32개)로 설치부터 제거까지 전 경로를 확인했다.
측정값과 날짜는 [`NEXT-SESSION.md`](NEXT-SESSION.md) 에 있다.

- [x] `wsl --import` / `--unregister` 라이프사이클 — import 8초
- [x] Rust 프로세스 실행 + stdout 스트리밍 + 그룹 종료 — 취소 시 잔존 프로세스 0
- [x] 한글/공백/괄호 경로 처리
- [x] rootfs 빌드 스크립트 — 503MB 이미지 생성, 인스톨러 동봉
- [x] 여덟 모듈 전부 앱에서 완주
- [x] 진행률 파싱 — 실측 로그로 교정, 테스트가 그 로그를 재생한다
- [x] 앱을 닫아도 작업이 계속되고 다시 켜면 이어받는지
- [x] 인스톨러 설치 → 온보딩 → 제거 시 배포판·데이터 정리
- [x] MCP — ChatGPT 데스크톱 앱 실접속, 공식 Inspector 로 규격 확인
- [ ] `/mnt/c` vs ext4 실행 시간 차이 — 구조의 전제이지만 아직 재보지 않았다
- [ ] perUser 인스톨러에서 권한 상승 헬퍼로 `wsl --install` 기동
      — 개발 PC 에 WSL 이 이미 있어 이 경로를 밟지 못했다
- [ ] 가상화 비활성화 기기에서 하드웨어 게이트가 재부팅 이전에 차단하는지
- [ ] 다른 PC 에서의 설치 — 이 PC 에서만 설치·제거를 확인했다
