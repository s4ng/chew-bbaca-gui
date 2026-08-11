# chewBBACA Desktop GUI

터미널 경험이 없는 연구자를 위한 [chewBBACA](https://github.com/B-UMMI/chewBBACA) 데스크톱 앱.
세균 cg/wgMLST 스키마 생성과 allele calling 을 클릭으로 실행한다.

> **상태: 초기 개발 (v0.1 진행 중).** 골격과 계층 배선은 올라와 있고,
> 실제 데이터셋 완주 검증은 아직이다. [검증되지 않은 가정](#검증되지-않은-가정) 참조.

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

설계의 전문은 [`ARCHITECTURE.md`](ARCHITECTURE.md), 진행 상황과 다음 작업은
[`doc/NEXT-SESSION.md`](doc/NEXT-SESSION.md) 에 있다.

---

## 사용자 요구사항

- Windows 10/11 (x86_64)
- CPU 가상화 활성화 (BIOS/UEFI)
- WSL2 — 없으면 앱이 온보딩에서 설치를 안내한다
- 디스크 여유 공간 10GB 이상 (rootfs 400~800MB + 분석 산출물)

관리자 권한은 **WSL 설치 단계에서만** 필요하다. 앱 본체는 현재 사용자 권한으로 설치·실행된다.

환경을 미리 점검하려면 저장소의 [`scripts/check-env.bat`](scripts/check-env.bat) 을 더블클릭하면 된다.

---

## 설치하기 (앱을 받은 분)

인스톨러 하나만 있으면 됩니다. **관리자 권한도, 인터넷도 필요 없습니다** —
분석 엔진이 인스톨러 안에 함께 들어 있습니다 (그래서 파일이 510MB 입니다).

### 1. 설치

`chewBBACA Desktop_0.1.0_x64-setup.exe` 를 실행합니다.

> **"Windows의 PC 보호" 경고가 뜹니다.** 코드 서명 인증서를 붙이지 않아서이며,
> 파일에 문제가 있다는 뜻은 아닙니다.
> **[추가 정보]** 를 누른 뒤 나타나는 **[실행]** 을 누르세요.

설치 위치는 `%LOCALAPPDATA%\chewBBACA Desktop\` 이고 다른 프로그램에 영향을 주지 않습니다.

### 2. 첫 실행

처음 켜면 실행 환경을 준비하는 화면이 나옵니다. 대부분 **[설치]** 한 번이면 끝나고
1분 이내에 완료됩니다. Windows 기능(WSL)이 없는 PC 라면 안내에 따라 한 번 재부팅해야
할 수 있습니다.

이미 다른 Linux 환경(WSL)을 쓰고 있어도 괜찮습니다. 이 앱은 `chewie-env` 라는
전용 환경만 만들고 기존 것은 건드리지 않습니다.

### 3. 무엇부터 해볼지

앱 왼쪽 아래 **[따라해보기]** 를 누르면 공개 예제 데이터로 전 과정을 따라가는
안내서가 열립니다. 용어 사전도 함께 들어 있습니다.

### 지울 때

설정 → 앱 및 기능에서 제거합니다. 제거 창의
**[모든 데이터 삭제]** 를 체크하면 분석 환경과 만들어둔 스키마까지 함께 지워집니다.
스키마를 남기고 싶으면 체크하지 마세요.

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

### 2. 실행

```powershell
npm install
npm run tauri:dev      # 개발 모드 (프론트 HMR + Rust 자동 재빌드)
```

### 3. 빌드

```powershell
npm run tauri:build    # dist + NSIS 인스톨러 (perUser)
```

### 4. 검증

```powershell
npm run typecheck              # 프론트엔드 타입 검사
cargo test --manifest-path src-tauri/Cargo.toml
```

---

## 저장소 구조

```
├── ARCHITECTURE.md          설계 문서 (이 저장소의 기준 문서)
├── CLAUDE.md                코드 작업 시 지켜야 할 규칙
├── doc/                     핸드오프 문서
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
    └── src/
        ├── runner/          ★ 이식 경계 — WSL 특화 로직은 여기까지만
        ├── env/             환경 검사 게이트 + 배포판 프로비저닝
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

만들어진 tar.gz 를 GitHub Releases 에 올리고, SHA256 을
`src-tauri/src/settings.rs` 의 기본값(또는 앱 [설정] 화면)에 넣는다.
체크섬이 비어 있으면 앱은 자동 다운로드를 시도하지 않고 안내만 표시한다.

---

## 기능 범위

| 릴리스 | 범위 |
| --- | --- |
| **v0.1 (완료)** | 온보딩/환경 구성, `CreateSchema`·`AlleleCall`·`ExtractCgMLST`, 실시간 로그·진행 표시·취소, 스키마 내보내기, 따라해보기 가이드 |
| v0.2 | `PrepExternalSchema`(외부 스키마 들여오기), 스키마 불러오기, `RemoveGenes`·`JoinProfiles`, `SchemaEvaluator`·`AlleleCallEvaluator` |
| v0.3 | `DownloadSchema`, `UniprotFinder` 주석 |

`ExtractCgMLST` 는 v0.2 예정이었으나 앞당겼다. 그것이 없으면 AlleleCall 의
`--gl` 칸을 채울 방법이 앱 안에 없어 워크플로가 닫히지 않는다.

평가 리포트 두 개는 v0.3 예정이었으나 v0.2 로 당겼다. 다만 **내장 뷰어는 만들지
않는다** — 리포트는 확대·검색·인쇄가 되는 기본 브라우저에서 여는 편이 낫다.
[작업 상세] 의 [리포트 열기] 가 회수된 HTML 을 브라우저로 넘긴다.

**범위 제외:** macOS/Linux 지원, Docker 실행, 클러스터/HPC 연동, Chewie-NS 쓰기 계열 기능.

---

## 검증 상태

2026-08-10 에 튜토리얼 데이터(*S. agalactiae* 완성 게놈 32개)로 설치부터 제거까지
전 경로를 실행해 확인했다. 측정값은 [`doc/NEXT-SESSION.md`](doc/NEXT-SESSION.md) 에 있다.

- [x] `wsl --import` / `--unregister` 라이프사이클 — import 8초
- [x] Rust 프로세스 실행 + stdout 스트리밍 + 그룹 종료 — 취소 시 잔존 프로세스 0
- [x] 한글/공백/괄호 경로 처리
- [x] rootfs 빌드 스크립트 — 503MB 이미지 생성, 인스톨러 동봉
- [x] 실제 데이터셋으로 CreateSchema → AlleleCall → ExtractCgMLST 완주
      — cgMLST(0.95) 1,270 loci, 예제 정답 1,267 과 일치
- [x] 진행률 파싱 — 실측 로그로 교정, 테스트가 그 로그를 재생한다
- [x] 앱을 닫아도 작업이 계속되고 다시 켜면 이어받는지
- [x] 인스톨러 설치 → 온보딩 → 제거 시 배포판·데이터 정리
- [ ] `/mnt/c` vs ext4 실행 시간 차이 — 구조의 전제이지만 아직 재보지 않았다
- [ ] perUser 인스톨러에서 권한 상승 헬퍼로 `wsl --install` 기동
      — 개발 PC 에 WSL 이 이미 있어 이 경로를 밟지 못했다
- [ ] 가상화 비활성화 기기에서 하드웨어 게이트가 재부팅 이전에 차단하는지

---

## 라이선스와 인용

chewBBACA 는 **GPLv3** 이다. 본 프로젝트는 별도 프로세스로 호출하는 래퍼이며 오픈소스로
공개한다. rootfs 에 GPL 소프트웨어를 포함해 배포하므로 각 패키지의 upstream 소스 취득
경로를 릴리스 노트에 명시한다.

> Mamede R, Vila-Cerqueira P, Carriço JA, Ramirez M. 2026. chewBBACA 3: lowering the barrier
> for scalable and detailed whole- and core-genome multilocus sequence typing.
> *Genome Med* 18:51.

- chewBBACA 문서 — https://chewbbaca.readthedocs.io
- Chewie-NS (스키마 저장소) — https://chewbbaca.online
