# chewBBACA Desktop GUI — 아키텍처

**대상 플랫폼:** Windows 10/11 (x86_64)
**백엔드 실행 환경:** WSL2 전용 배포판 (`chewie-env`)
**문서 상태:** v0.1 구현 완료, 튜토리얼 데이터로 전 과정 검증
**최종 수정:** 2026-08-10

이 문서는 시스템의 구조 — 계층, 컴포넌트 경계, 데이터 흐름, 수명주기 — 를 기술한다.
현재 진행 상황과 다음 작업은 [`doc/NEXT-SESSION.md`](doc/NEXT-SESSION.md) 에 있다.

---

## 1. 시스템 개요

chewBBACA는 세균 cg/wgMLST 스키마 생성 및 allele calling을 수행하는 CLI 전용 소프트웨어다.
본 프로젝트는 터미널 경험이 없는 연구자를 위해 이를 데스크톱 GUI로 감싼다.

**아키텍처를 지배하는 단 하나의 제약:**

> chewBBACA는 Windows에서 네이티브로 실행할 수 없다.
> (Bioconda가 Linux/macOS만 지원하고, BLAST+·MAFFT·FastTree 조달 및 경로 처리가 Windows에서 미검증)

따라서 본 시스템은 **Windows GUI 프로세스**와 **Linux 실행 환경** 사이의 다리(bridge)이며,
아키텍처 복잡도의 대부분은 그 경계 — 경로 변환, 프로세스 제어, 파일 이동 — 에 집중된다.

### 1.1 설계 원칙

| 원칙 | 의미 |
| --- | --- |
| **사용자 환경 불가침** | 기존 WSL 배포판·`.wslconfig`·전역 설정을 수정하지 않는다. 전용 배포판만 소유한다. |
| **경계 격리** | WSL 특화 로직은 Runner 계층 내부에만 존재한다. UI는 WSL의 존재를 모른다. |
| **작업은 앱보다 오래 산다** | 40분+ 실행되는 작업이 존재한다. 상태는 프로세스 메모리가 아니라 SQLite에 있다. |
| **I/O는 ext4에서** | 사용자 파일은 실행 전 WSL 내부로 복사한다. `/mnt/c` 위에서 연산하지 않는다. |
| **정상 사용자에게 묻지 않는다** | 환경 검사는 실패했을 때만 진단 경로로 내려간다. |

---

## 2. 계층 구조

```
┌───────────────────────────────────────────────────────────────┐
│  Presentation — React + TypeScript (WebView2)                 │
│  마법사형 작업 폼 · 실시간 로그 뷰 · 스키마 관리 · HTML 리포트 뷰어  │
└───────────────────────────┬───────────────────────────────────┘
                            │  Tauri IPC (command / event)
┌───────────────────────────▼───────────────────────────────────┐
│  Application Core — Rust (Tauri 2)                            │
│  ┌──────────────┬──────────────┬───────────────┬────────────┐ │
│  │ Job Manager  │ Environment  │ Schema Store  │ Settings   │ │
│  │              │ Provisioner  │               │            │ │
│  └──────────────┴──────────────┴───────────────┴────────────┘ │
│  ┌──────────────────────────┐  ┌───────────────────────────┐  │
│  │ Persistence (SQLite)     │  │ Log Sink (파일)            │  │
│  └──────────────────────────┘  └───────────────────────────┘  │
└───────────────────────────┬───────────────────────────────────┘
                            │  trait ChewieRunner  ◀── 이식 경계
┌───────────────────────────▼───────────────────────────────────┐
│  Runner — WslRunner (현재 유일한 구현체)                        │
│  경로 변환(wslpath) · 프로세스 그룹 · stdout 스트리밍 · 파일 스테이징 │
└───────────────────────────┬───────────────────────────────────┘
                            │  wsl.exe -d chewie-env
┌───────────────────────────▼───────────────────────────────────┐
│  Backend — WSL2 전용 배포판 `chewie-env`                       │
│  micromamba(base) → chewBBACA 3.5.4 + BLAST+ / MAFFT / FastTree│
│  ext4 작업 공간: ~/work/{job_id}, ~/schemas/{schema_id}         │
└───────────────────────────────────────────────────────────────┘
```

**이식 경계(`ChewieRunner`)의 위쪽은 플랫폼 중립이다.** `wslpath`, `wsl --import`, PGID 같은
개념이 이 선을 넘어 위로 새어나가면 안 된다. macOS 확장 시 아래쪽만 교체된다(§9).

---

## 3. 기술 스택과 선정 근거

### 3.1 셸: Tauri 2 + React + TypeScript

| 요구사항 | Tauri가 충족하는 방식 |
| --- | --- |
| 작은 번들 | ~10MB. rootfs(400~800MB)가 이미 무거우므로 셸은 가벼워야 한다 |
| 인스톨러/언인스톨러 | `tauri-bundler`가 NSIS(.exe)·MSI(WiX)를 설정 하나로 생성 |
| 자동 업데이트 | `tauri-plugin-updater` 내장. 서명 키만 필요(코드 서명 인증서와 무관) |
| 실시간 로그 | Rust에서 자식 프로세스 stdout → 프론트 이벤트 전달이 자연스러움 |
| HTML 리포트 | SchemaEvaluator 산출물을 앱 내 WebView에 그대로 렌더 |

**탈락 대안:** Electron(용량·메모리), PyQt/Tkinter + PyInstaller(번들 앱의 외부 환경 관리 시 경로 충돌),
.NET MAUI/WPF(macOS 확장 차단).

### 3.2 백엔드 실행: WSL 직접 호출 (Docker 미사용)

| 항목 | WSL 직접 (채택) | Docker (미채택) |
| --- | --- | --- |
| 사용자 사전 설치 | WSL만 | Docker Desktop 추가 |
| 성능 레이어 | 1 | WSL2 + 컨테이너 |
| 프로세스 취소 | 직접 구현 (§6) | `docker stop` |
| 라이선스 | 없음 | 기업 사용 시 유료 |

설치 단계를 하나 줄이는 것이 최우선이므로 WSL 직접 방식을 택하고, 프로세스 제어 복잡도를 감수한다.
Docker Desktop도 내부적으로 WSL2 백엔드를 쓰므로 **이 선택으로 추가된 사용자 부담은 없다.**

### 3.3 영속성: SQLite (`tauri-plugin-sql` 또는 rusqlite)

작업 메타데이터·실행 이력·설정·스키마 목록을 보관한다. 앱 재시작 후 진행 중이던 작업을
복구하기 위한 필수 컴포넌트다(§6.3).

---

## 4. 컴포넌트

### 4.1 `ChewieRunner` (이식 경계)

```rust
trait ChewieRunner {
    fn ensure_ready(&self) -> Result<()>;
    fn run(&self, module: Module, args: Args) -> Result<JobHandle>;
    fn cancel(&self, handle: &JobHandle) -> Result<()>;
    fn to_backend_path(&self, host: &Path) -> Result<String>;
}
```

`WslRunner`가 유일한 구현체이며, 다음을 캡슐화한다.

- **경로 변환** — `wslpath -a`에 위임. `C:\` → `/mnt/c/` 문자열 치환을 직접 구현하지 않는다.
- **프로세스 기동** — `wsl.exe -d chewie-env` + `setsid`로 새 프로세스 그룹 생성
- **stdout/stderr 스트리밍** — 라인 단위로 Job Manager에 전달
- **파일 스테이징** — 입력 복사(Windows → ext4), 결과 회수(ext4 → Windows)

### 4.2 Job Manager

작업의 수명주기(§6)를 소유하는 단일 지점. 동시 실행은 기본 1건으로 직렬화한다
(chewBBACA가 `--cpu`로 이미 전 코어를 점유하므로 병렬 실행은 이득이 없다).

책임:
- 작업 큐잉 및 상태 전이 기록
- 로그 라인을 파일로 기록하고 동시에 UI 이벤트로 방출
- 진행률 파싱 (chewBBACA 출력 패턴 기반)
- 취소 요청을 PGID 종료로 변환
- 앱 시작 시 고아 프로세스 조정(reconciliation)

### 4.3 Environment Provisioner

`chewie-env` 배포판의 설치·검증·제거·업데이트를 담당한다(§7, §8.2).
rootfs 확보(동봉본/로컬/원격 — §8.1), SHA256 검증, `wsl --import`, 디스크 정리를 수행한다.

### 4.4 Schema Store

**스키마는 WSL 내부(`~/schemas/`)에 상주하며 앱이 소유한다.**
AlleleCall이 신규 allele을 스키마 디렉터리에 계속 추가하므로, Windows 측에 두면
9p 오버헤드가 실행마다 누적된다. GUI는 목록·삭제·내보내기(export) 인터페이스만 제공한다.

각 스키마는 Prodigal training file(`.trn`)을 내부에 보관한다. AlleleCall 시 `--ptf`를
다시 넘기지 않으며, **결과 일관성을 위해 동일 training file을 계속 사용해야 한다.**

### 4.5 Report Viewer

SchemaEvaluator / AlleleCallEvaluator가 생성하는 인터랙티브 HTML을 표시한다.
환경 구성에 실패한 사용자를 위한 **결과 뷰어 모드**(외부 PC에서 생성된 리포트·TSV 열람)로도
동작한다(§7.7).

> **2026-08-11 결정: 앱 내 WebView 로 띄우지 않고 기본 브라우저로 넘긴다.**
> 내장하려면 CSP 를 열고 asset 프로토콜을 붙여야 하는데, 정작 리포트는 확대·검색·
> 인쇄가 되는 브라우저에서 보는 편이 낫다. [작업 상세]의 [리포트 열기]가
> 회수된 폴더에서 `*_report.html` 을 찾아 `opener` 로 연다(`commands::report_open`).
> 결과 뷰어 모드는 이 결정과 무관하게 남아 있다.

---

## 5. 데이터 흐름과 파일 배치

### 5.1 실행 시퀀스

```
사용자 입력 폼
     │
     ▼
Job Manager ── job 레코드 생성 (status=queued) ──▶ SQLite
     │
     ▼
WslRunner.run()
     │
     ├─ 1. 입력 스테이징 : C:\...\assemblies\*.fasta
     │                    →  ~/work/{job_id}/input/     [ext4]
     │
     ├─ 2. setsid 로 프로세스 그룹 생성, PGID 획득
     │       └─ PGID → SQLite + WSL 내부 파일에 이중 기록
     │
     ├─ 3. chewBBACA.py <Module> ... --cpu $(nproc)
     │       실행 경로 전체가 ext4 내부
     │
     ├─ 4. stdout ─(라인)─▶ 로그 파일
     │                  └─▶ Tauri 이벤트 ─▶ React 로그 뷰
     │
     └─ 5. 결과 회수 : ~/work/{job_id}/output/ 중 사용자 요청분만
                      →  C:\Users\...\결과폴더\
     │
     ▼
status=completed | failed | cancelled  ──▶ SQLite
```

### 5.2 파일시스템 성능 — 최우선 고려사항

WSL2에서 `/mnt/c` 접근은 9p 프로토콜을 경유하며 **네이티브 ext4 대비 5~20배 느리다.**
chewBBACA는 수백 개 FASTA를 읽고 loci FASTA 수천 개를 쓰므로 이 차이가 치명적이다.

**규칙:**
1. 사용자 입력을 `~/work/{job_id}/input`으로 복사한다.
2. 모든 실행은 WSL 내부 경로에서만 수행한다.
3. 결과 중 사용자가 필요로 하는 파일만 Windows로 회수한다.

복사 오버헤드가 있더라도 전체 실행 시간은 크게 단축된다. (첫 스프린트에서 실측 검증 — §10)

### 5.3 디렉터리 레이아웃

**Windows 측**

```
%LOCALAPPDATA%\ChewieApp\
├── wsl\ext4.vhdx          # chewie-env 배포판 실체
├── app.db                 # SQLite
├── logs\{job_id}.log      # 작업 로그 (DB에는 경로만 저장)
└── cache\rootfs-*.tar.gz  # 원격 rootfs 를 쓸 때만 생기는 다운로드 캐시 (§8.1)
```

`%LOCALAPPDATA%`를 사용한다. `C:\ProgramData`·`Program Files`는 관리자 권한이 필요해
설치 경험을 해친다.

**WSL(`chewie-env`) 측**

```
~/
├── work/{job_id}/{input,output}   # 작업별 임시 공간
└── schemas/{schema_id}/           # 앱이 소유하는 영구 스키마
    ├── schema_seed/
    └── *.trn
```

### 5.4 경로 변환

`wslpath -a`에 위임하고, 다음 케이스를 초기부터 테스트한다.

- 한글이 포함된 경로
- 공백이 포함된 경로
- OneDrive 동기화 폴더 (`C:\Users\xxx\OneDrive\...`)
- UNC 경로 / 네트워크 드라이브 — **미지원으로 명시하고 입력 단계에서 차단**

---

## 6. 작업 수명주기와 프로세스 제어

### 6.1 상태 모델

```
queued ──▶ running ──┬──▶ completed
                     ├──▶ failed
                     └──▶ cancelled
```

SQLite `jobs` 테이블에 보관하는 항목:

| 컬럼 | 용도 |
| --- | --- |
| `job_id`, `module`, `args` | 재현 및 이력 |
| `started_at`, `finished_at` | 소요 시간 |
| `status` | 위 상태 모델 |
| `pgid` | 취소 및 고아 프로세스 판정 |
| `work_dir` | WSL 내부 작업 디렉터리 |
| `log_path` | 로그는 파일로, DB에는 경로만 |
| `output_path` | 결과물 위치 |

### 6.2 취소 — 좀비 프로세스 문제

> **`wsl.exe` 자식 프로세스를 kill해도 내부 Linux 프로세스는 살아남는다.**
> BLAST가 CPU를 점유한 채 남아 사용자 시스템을 마비시킬 수 있다.

**구현 방침:**
1. 실행 시 `setsid`로 새 프로세스 그룹을 만든다.
2. 그룹 PID를 WSL 내부 파일과 SQLite에 함께 기록한다.
3. 취소 시 **별도 프로세스**로 `wsl.exe -d chewie-env kill -- -{PGID}`를 호출해 그룹 전체를 종료한다.

### 6.3 앱 시작 시 조정(reconciliation)

앱은 종료되어도 WSL 내부 작업은 계속 실행된다. 시작 시 `status=running`인 레코드에 대해
PGID 생존 여부를 확인하고 다음 중 하나로 수렴시킨다.

- **생존** → 사용자에게 "이전 작업이 실행 중입니다 — 복구 / 종료" 선택 제시
- **사망 + 결과 존재** → `completed`로 확정
- **사망 + 결과 없음** → `failed`로 표시

### 6.4 실행 환경 세부 설정

| 항목 | 설정 | 이유 |
| --- | --- | --- |
| 환경변수 | `WSL_UTF8=1` | 미설정 시 `wsl --list` 등이 UTF-16LE를 출력해 파싱이 깨진다 |
| 프로세스 플래그 | `CREATE_NO_WINDOW` (`0x08000000`) | Rust `Command`의 `creation_flags`로 지정. 미설정 시 검은 콘솔 창이 반복 노출된다 |
| 환경변수 | `PYTHONUNBUFFERED=1` | tty가 아니어서 출력이 버퍼링된다. 실시간 진행률 표시에 필수 |
| CPU 개수 | WSL 내부 `nproc` 기준 | Windows 논리 코어 수와 다를 수 있다. `--cpu` 인자에 이 값을 사용 |

### 6.5 디스크 공간

**`ext4.vhdx`는 파일을 삭제해도 자동으로 축소되지 않는다.** 대용량 분석 후 Windows 여유 공간이
회복되지 않는다.

- 설정 화면에 **"디스크 정리"** 버튼 제공
- `wsl --manage chewie-env --set-sparse true` 또는 `diskpart`의 `compact vdisk` 호출
- 실행 전 `wsl --terminate chewie-env` 필요

**`.wslconfig`는 앱이 수정하지 않는다.** 전역 설정이므로 사용자의 다른 배포판에 영향을 준다.
메모리 제한이 필요하면 안내만 한다.

---

## 7. 환경 부트스트랩 (첫 실행 온보딩)

가장 많은 이탈이 발생하는 구간이다. 이 절의 설계 목표는 두 가지다 —
**정상 환경 사용자에게는 아무것도 묻지 않을 것**, 그리고
**돌이킬 수 없는 조작(기능 활성화·재부팅)을 하기 전에 불가능한 기기를 먼저 걸러낼 것.**

### 7.1 부트스트랩은 인스톨러가 아니라 앱이 수행한다

WSL 설치를 인스톨러 단계로 옮기는 방안(Docker Desktop 방식)을 검토했으나 채택하지 않았다.

| 항목 | 인스톨러에서 설치 (미채택) | 앱 온보딩에서 설치 (채택) |
| --- | --- | --- |
| 인스톨러 권한 | perMachine + 관리자 필수 | 현재 사용자, 관리자 불필요 |
| 설치 위치 | `Program Files` 계열 강제 | `%LOCALAPPDATA%` 유지 (§5.3) |
| 실패 시 UI | NSIS 커스텀 액션 — 안내 화면 불가 | 전체 React UI 사용 가능 |
| BIOS 문제 감지 | 불가 (§7.2 참조) | 재부팅 **이전에** 감지 |
| 실제 단계 수 | 설치 → 재부팅 → 첫 실행 → rootfs | 설치 → 첫 실행 → 재부팅 → rootfs |

핵심은 **단계 수가 줄지 않는다**는 점이다. WSL 기능 활성화는 어차피 재부팅을 요구하므로
인스톨러로 옮겨도 재부팅과 `wsl --import`는 그대로 남는다. 반면 실패를 안내할 UI는 잃는다.
rootfs **파일**은 인스톨러에 담지만(§8.1), 배포판 **등록**은 앱이 한다 — 담는 것과
실행하는 것은 별개다.

### 7.2 Docker Desktop 방식을 따르지 않는 이유

Docker Desktop 인스톨러는 WSL 기능을 직접 활성화하지만, **BIOS 가상화가 꺼진 기기에서
설치는 그대로 성공한다.** 기능 활성화에 쓰이는 DISM은 선택적 구성 요소를 스테이징할 뿐
CPU 가상화 지원 여부를 검사하지 않기 때문이다. 하드웨어 요구사항은 하이퍼바이저가 실제로
VM을 기동하는 순간에야 평가된다.

결과적으로 사용자는 다음 경로를 밟는다.

```
설치 성공 ✓ → 재부팅 → 첫 실행 → ✗ 0x80370102
                                    "The virtual machine could not be started
                                     because a required feature is not installed"
```

`0x80370102`는 BIOS 가상화 비활성화 · VMP 미활성화 · 중첩 가상화 미노출 ·
`hypervisorlaunchtype off` · 타 하이퍼바이저 충돌을 모두 포괄하는 코드라 원인을 특정해주지
못한다. **즉 이 방식은 재부팅을 시킨 뒤에야, 그것도 판독 불가능한 코드로 실패한다.**

본 프로젝트는 하드웨어 게이트를 앞으로 당겨(§7.3) 이 경로를 회피한다.

### 7.3 부트스트랩 흐름

정상 사용자를 위해 **실제 실행을 먼저 시도하고**, 실패했을 때만 게이트를 순서대로 내려간다.
각 게이트는 이전 게이트가 통과된 경우에만 부작용을 일으킨다.

```
앱 시작
  │
  ▼
wsl -d chewie-env -- true                    ◀── 낙관적 시도
  │
  ├─ 성공 ─────────────────────────────────▶ 앱 진입 (질문 없음)
  │
  └─ 실패
       │
       ▼
  ① 하드웨어 게이트   HypervisorPresent
       │
       ├─ false ─▶ VirtualizationFirmwareEnabled 보조 확인
       │           └─▶ BIOS 가상화 안내 (§7.6) ─▶ 중단
       │               ※ 이 시점까지 기능 활성화·재부팅을 하지 않았다
       │
       └─ true
            │
            ▼
  ② WSL 게이트        wsl --status
            │
            ├─ 미설치 / WSL1 ─▶ WSL 설치 (§7.5) ─▶ 재부팅 ─▶ [앱 시작]으로 복귀
            │
            └─ 정상
                 │
                 ▼
  ③ 배포판 게이트     rootfs 확보(동봉본) → SHA256 검증 → wsl --import
                 │
                 └──────────────────────────────▶ 앱 진입
```

이 흐름은 **재진입 가능(idempotent)** 하다. 재부팅 후 앱이 다시 시작되면 동일한 낙관적 시도로
진입해 자연스럽게 다음 게이트로 이어진다. 중단된 지점을 별도로 기억할 필요가 없다.

### 7.4 환경 검사 신호

| 검사 | 명령 | 비고 |
| --- | --- | --- |
| 하이퍼바이저 동작 | `(Get-CimInstance Win32_ComputerSystem).HypervisorPresent` | **1차 판정 기준** |
| 펌웨어 가상화 | `(Get-CimInstance Win32_Processor).VirtualizationFirmwareEnabled` | 보조 신호. 단독 사용 금지 (아래) |
| WSL 설치/버전 | `wsl --status` | `WSL_UTF8=1` 필요 (§6.4) |
| 배포판 존재 | `wsl -d chewie-env -- true` | 낙관적 시도의 진입점 |

> **`VirtualizationFirmwareEnabled`를 단독 판정에 쓰지 않는다.**
> 이 속성은 하이퍼바이저가 이미 실행 중일 때 `False`를 반환하는 것으로 알려져 있다
> (호스트 OS 자신이 하이퍼바이저 위에서 동작하므로 펌웨어 상태를 조회할 수 없다).
> 이 값만 보고 판정하면 **정상 동작하는 기기를 "가상화 꺼짐"으로 오진**한다.
> 반드시 `HypervisorPresent == false`인 경우에 한해 보조 신호로만 참조한다.

### 7.5 WSL 설치 — 권한 상승 헬퍼

`wsl --install`은 **관리자 권한과 재부팅**을 요구한다. 하드웨어 게이트(①)를 통과한 기기에
한해 앱이 대행한다.

```powershell
wsl --install --no-distribution
```

`--no-distribution`이 필수다. 생략하면 Ubuntu가 함께 설치되어 §4.1의 "사용자 환경 불가침"
원칙을 위반한다.

**구현 방침:**

1. UI에 **"WSL 설치" 버튼 하나**를 제공한다. 클릭 시 `runas` 동사로 권한 상승된 헬퍼
   프로세스를 기동하고, 앱 본체는 비관리자로 유지한다.
2. 완료 후 재부팅 안내를 표시한다. 재부팅 후 §7.3 흐름으로 자동 복귀한다.
3. **권한 상승이 거부되면 명령어 복사 버튼 + 관리자 PowerShell 실행 방법 안내로 폴백한다.**
   버튼을 제공하되, 수동 경로를 없애지는 않는다.
4. WSL1이 기본값인 경우 `wsl --set-default-version 2`를 함께 수행한다.

**폴백이 필요한 실패 사례:**

- 최신 빌드의 `wsl --install`은 Microsoft Store에서 WSL을 받는다. Store가 차단된 기업 장비에서
  실패하므로 `--inbox`(인박스 버전 사용) 또는 `wsl --update --web-download`를 대안으로 시도한다.
- 그룹 정책으로 선택적 기능 설치가 차단된 경우 — §7.7 대안으로 이동한다.

### 7.6 BIOS 가상화 안내

**Windows 11이라고 해서 가상화가 켜져 있다고 가정하면 안 된다.** Windows 11 최소 요구사항
(TPM 2.0, Secure Boot, 지원 CPU)에 가상화는 포함되지 않는다. 다음 경우 꺼져 있을 수 있다:
자가 조립 PC, Windows 10에서 업그레이드한 기기, 안티치트/성능 이슈로 사용자가 끈 경우,
기업 지급 장비.

이 화면은 하드웨어 게이트(§7.3 ①)에서만 도달하며, **이 시점까지 Windows 기능 활성화나 재부팅을
수행하지 않았다.** 사용자는 헛된 재부팅 없이 자기 기기의 상태를 알게 된다.

앱이 제공할 것:

1. **펌웨어 직접 진입 버튼** — 관리자 권한으로 `shutdown /r /fw /t 1`. 재부팅과 동시에 UEFI로
   진입하므로 "부팅 중 F2 연타" 장벽이 사라진다. (UEFI 한정, 레거시 BIOS 불가)
2. **제조사별 맞춤 안내** — `(Get-CimInstance Win32_ComputerSystem).Manufacturer`로 제조사를 읽어
   해당 기종의 진입 키와 메뉴 경로를 표시. 설정 항목명은 제조사마다 다르다
   (Intel: `Intel Virtualization Technology` / `VT-x` / `Vanderpool`, AMD: `SVM Mode`;
   위치: Advanced / CPU Configuration / M.I.T. / OC Tweaker 등).
3. **자가 확인 방법** — 작업 관리자 → 성능 → CPU → "가상화: 사용" 스크린샷.

### 7.7 실패 시 대안

BIOS에 관리자 암호가 걸린 회사 장비 등 끝내 불가능한 사용자가 반드시 발생한다.
"실행할 수 없습니다"로 끝내지 않는다.

- **Galaxy 웹 버전 안내** — usegalaxy.eu에 chewBBACA 모듈(CreateSchema, AlleleCall,
  DownloadSchema, PrepExternalSchema)이 등록되어 브라우저에서 실행 가능하다.
  단, 버전이 최신보다 뒤처질 수 있고 DownloadSchema에서 균종 매핑 오류 이력이 있다.
- **결과 뷰어 모드** — 다른 PC에서 생성된 HTML 리포트·TSV를 이 앱으로 열람(§4.5).

---

## 8. 빌드 및 배포

### 8.1 산출물 구성

| 산출물 | 크기 | 배포 시점 |
| --- | --- | --- |
| 인스톨러 (NSIS) — GUI + rootfs 동봉 | ~520MB | 설치 시 |

**rootfs를 인스톨러에 함께 담는다.** 배포 대상이 지인 소수이고 전달 수단이 zip이라
호스팅을 두는 편익이 없다. 대신 다음을 얻는다.

- 첫 실행에 인터넷이 필요 없다 — 온보딩 ③에서 네트워크 실패 경로가 사라진다.
- 앱 버전과 rootfs 버전이 **구조적으로** 어긋날 수 없다.
- URL 만료·호스팅 중단으로 과거 빌드가 죽는 일이 없다.

대가는 인스톨러 크기와, 설치 후에도 tar.gz가 설치 폴더에 남는다는 점이다
(언인스톨러가 추적하는 파일이라 앱이 임의로 지우지 않는다).

동봉 방식은 Tauri 리소스다. `dist-rootfs/`가 없는 상태에서도 개발이 되어야 하므로
**리소스 선언은 기본 설정이 아니라 오버레이 설정에 둔다.**

```
src-tauri/tauri.conf.json    ← GUI 만. tauri dev / tauri:build:slim 이 쓴다
src-tauri/tauri.bundle.json  ← resources 매핑만. npm run tauri:build 가 --config 로 덧씌운다
  "../dist-rootfs/chewie-rootfs-3.5.4.tar.gz" → "$RESOURCE/rootfs/chewie-rootfs-3.5.4.tar.gz"
```

NSIS 압축은 `none`이다. tar.gz는 이미 압축되어 있어 LZMA를 한 번 더 돌려도
크기는 거의 그대로인데 빌드 시간만 수십 분 늘어난다.

런타임 해석은 `Provisioner::rootfs_origin()` 하나로 모인다.

| origin | 조건 | 동작 |
| --- | --- | --- |
| `bundled` | 설정 URL 이 비어 있고 리소스에 파일이 있음 | 제자리 해싱 → import (기본 경로) |
| `localFile` | 설정 URL 이 로컬 경로 | 제자리 해싱 → import |
| `remote` | 설정 URL 이 http(s) | 다운로드 → 검증 → import |
| `missing` | 둘 다 없음 (개발 실행) | 설치 버튼을 숨기고 안내 |

설정의 URL이 동봉본을 **이긴다.** 동봉본을 두고 URL을 채우는 유일한 이유가
"다른 이미지를 시험하려는 것"이기 때문이다. 원격 경로 코드는 남겨 둔다 —
사설망 배포나 rootfs만 따로 갱신해야 할 때 되살아난다.

**인스톨러는 현재 사용자(perUser) 모드로 동작하며 관리자 권한을 요구하지 않는다.**
WSL 설치·Windows 기능 활성화는 인스톨러가 아니라 앱 온보딩의 권한 상승 헬퍼가 수행한다(§7.1).

### 8.2 rootfs 빌드 파이프라인 (CI)

```dockerfile
# rootfs 생성 전용 — 런타임에 Docker를 쓰는 것이 아님
FROM mambaorg/micromamba:1.5-jammy
USER root
RUN micromamba install -y -n base -c conda-forge -c bioconda \
      chewbbaca=3.5.4 && \
    micromamba clean -a -y
```

```bash
docker build -t chewie-rootfs .
docker create --name tmp chewie-rootfs
docker export tmp | gzip > chewie-rootfs-3.5.4.tar.gz
docker rm tmp
```

> Docker는 **빌드 타임에만** 사용한다. 사용자 PC에는 Docker가 전혀 필요하지 않다.

이미지 요구사항:
- `/etc/wsl.conf`에 기본 사용자와 `[interop]` 설정 포함
- micromamba 환경이 non-login shell에서도 활성화되도록 `.bashrc` 또는 wrapper 스크립트 준비
- SHA256 체크섬을 `settings.rs`의 기본값에 반영 (동봉본도 매번 검증한다)

`build.sh`는 산출물을 `dist-rootfs/`에 둔다. **인스톨러가 집어가는 위치가 그곳이므로
경로와 파일명을 바꾸면 `tauri.bundle.json`도 함께 고쳐야 한다.** rootfs를 다시 빌드하면
`docker build`에 재현성이 없어 체크섬이 반드시 바뀌므로, `settings.rs`의 기본 체크섬을
갱신하지 않으면 온보딩 ③이 검증 단계에서 실패한다.

### 8.3 배포판 수명주기

```
설치   wsl --import chewie-env "%LOCALAPPDATA%\ChewieApp\wsl" rootfs.tar --version 2
업데이트  rootfs 이미지 통째 교체 (스키마는 사전 백업/이관 필요)
제거   wsl --unregister chewie-env
```

전용 배포판을 별도 등록하는 이유: 사용자 기존 환경과의 버전 충돌 원천 차단,
그리고 **언인스톨이 한 줄로 완결된다는 점** — 이것이 가장 크다.

#### 언인스톨러가 지우는 것

설치 위치는 `%LOCALAPPDATA%\chewBBACA Desktop\` 이고 (perUser), 그 안에
`chewie-app.exe`·`rootfs\*.tar.gz`·`uninstall.exe` 가 들어간다. 제거 시 이 셋과
바로가기·HKCU 레지스트리 키가 사라진다.

문제는 **앱이 만드는 것들**이다. `%LOCALAPPDATA%\ChewieApp\`(§5.3)과 등록된
`chewie-env` 배포판은 인스톨러가 모르는 존재다. NSIS 의 "앱 데이터 삭제" 체크박스는
BUNDLEID 경로(`io.github.chewbbaca.desktop` — 실제로는 WebView2 프로필뿐)만 지우므로,
그대로 두면 사용자가 체크하고도 수 GB 짜리 `ext4.vhdx` 와 배포판 등록이 남는다.

그래서 `src-tauri/nsis/hooks.nsh` 의 `NSIS_HOOK_POSTUNINSTALL` 이 체크박스가 켜졌을 때만
`wsl --unregister chewie-env` → `RMDir /r %LOCALAPPDATA%\ChewieApp` 을 수행한다.

- **`PRE` 가 아니라 `POST` 다.** `PREUNINSTALL` 은 `CheckIfAppIsRunning` 앞에서 돌아,
  앱이 아직 떠 있고 작업이 실행 중인 상태에서 배포판을 날릴 수 있다.
- **순서를 뒤집으면 안 된다.** 폴더를 먼저 지우면 vhdx 를 잃은 배포판 등록만 남는다.
- **업데이트(`/UPDATE`)에서는 타지 않는다.** 버전만 올리려다 스키마가 날아간다.
- 32비트 NSIS 에서 `$WINDIR\System32` 는 SysWOW64 로 리다이렉트되어 `wsl.exe` 를 찾지
  못한다. `${DisableX64FSRedirection}` 이 필수다.
- 체크박스 문구는 `nsis/Korean.nsh`·`English.nsh` 로 교체했다. 기본 문구
  "애플리케이션 데이터 삭제하기"는 몇 시간짜리 스키마가 함께 사라진다는 사실을 숨긴다.

**`.nsh` 파일의 BOM 규칙이 파일마다 반대다.** `hooks.nsh` 는 원본 경로에서 그대로
`!include` 되므로 UTF-8 BOM 이 **있어야** makensis 가 한글을 UTF-8 로 읽는다(없으면 ACP=CP949
로 오독). 반면 언어 파일은 Tauri 가 `target\release\nsis\x64\` 로 복사하며 BOM 을 스스로
붙이므로 원본에 BOM 이 **있으면 안 된다** — 두 번 들어가 `Invalid command: ";"` 로 죽는다.

체크박스를 끄면 예전처럼 남는다 — 재설치 후 스키마를 이어 쓰려는 사용자의 경로다.

### 8.4 코드 서명

**초기 릴리스에서는 생략한다.** 오픈소스 + 지인 대상이므로 EV 인증서 비용을 감수하지 않는다.

- 첫 설치 시 SmartScreen 경고가 표시된다. README에 "추가 정보 → 실행" 스크린샷을 반드시 포함한다.
- Tauri updater의 서명 키는 코드 서명 인증서와 무관하므로 **자동 업데이트는 정상 동작한다.**

### 8.5 라이선스

chewBBACA는 **GPLv3**다. 본 프로젝트는 별도 프로세스로 호출하는 래퍼이며 자체도 오픈소스로
공개하므로 충돌 소지가 없다. 다만 rootfs에 GPL 소프트웨어를 포함해 배포하므로
**각 패키지의 upstream 소스 취득 경로를 문서에 명시**한다.

인용 정보는 앱 내 About 화면과 README에 표기한다.

> Mamede R, Vila-Cerqueira P, Carriço JA, Ramirez M. 2026. chewBBACA 3: lowering the barrier
> for scalable and detailed whole- and core-genome multilocus sequence typing. Genome Med 18:51.

---

## 9. 이식성 — macOS 확장 대비

현재 범위는 Windows 전용이나, `ChewieRunner` 경계(§4.1)를 지키면 확장 비용이 크게 줄어든다.

| | Windows (`WslRunner`) | macOS (`NativeRunner`, 미구현) |
| --- | --- | --- |
| 실행 환경 | WSL2 전용 배포판 | micromamba 직접 설치 |
| 경로 변환 | `wslpath -a` | 불필요 |
| 프로세스 그룹 | `setsid` + PGID 원격 kill | 로컬 프로세스 그룹 |
| 디스크 관리 | vhdx 압축 필요 | 불필요 |

macOS는 Bioconda 네이티브 지원이 되므로 WSL 관련 복잡도가 통째로 사라져 **오히려 단순하다.**
UI 레이어는 trait 뒤에서만 동작하도록 하고, WSL 특화 로직이 프론트엔드로 새어나가지 않게 한다.

---

## 10. 기능 범위 (릴리스 로드맵)

chewBBACA 워크플로우:

```
1. CreateSchema      — 어셈블리로부터 wgMLST 스키마 생성
2. AlleleCall        — 균주별 allelic profile 결정, 신규 allele을 스키마에 추가
3. ExtractCgMLST     — 결과로부터 core genome loci 집합 확정
4. SchemaEvaluator / AlleleCallEvaluator — 인터랙티브 HTML 리포트
```

| 릴리스 | 범위 |
| --- | --- |
| **v0.1 (MVP)** | 온보딩/환경 구성, `CreateSchema`·`AlleleCall`, 실시간 로그·진행 표시·취소, 결과 폴더 열기/내보내기 |
| **v0.2** | `ExtractCgMLST`·`RemoveGenes`·`JoinProfiles`, 리포트 내장 뷰어, 스키마 관리 화면 |
| **v0.3** | `DownloadSchema`, `PrepExternalSchema`, `UniprotFinder` 주석 |

**범위 제외:** macOS/Linux 지원, Docker 실행, 클러스터/HPC 연동,
Chewie-NS 쓰기 계열 원격 기능(`LoadSchema`).

### 10.1 CLI 매핑 (참조)

```bash
# 스키마 생성 (CDS 입력 시 --cds 추가)
chewBBACA.py CreateSchema -i <어셈블리폴더> -o <스키마폴더> --ptf <종.trn> --cpu <N>

# Allele calling (일부 loci만: --gl <loci목록.txt>)
chewBBACA.py AlleleCall -i <어셈블리폴더> -g <스키마폴더>/schema_seed -o <결과폴더> --cpu <N>

# core genome 추출
chewBBACA.py ExtractCgMLST -i <results_alleles.tsv> -o <출력> --t 0.95
```

chewBBACA v2로 만든 스키마는 `PrepExternalSchema`로 변환이 필요하다.

---

## 11. 검증되지 않은 가정

아키텍처가 의존하지만 아직 실측되지 않은 항목. **GUI보다 먼저** 프로토타입으로 검증한다.

- [ ] `/mnt/c` vs ext4 실행 시간 차이 (§5.2의 전제)
- [ ] `wsl --import` / `--unregister` 라이프사이클
- [ ] Rust에서 프로세스 실행 + stdout 스트리밍 + 그룹 종료 (§6.2)
- [ ] 한글/공백 경로 처리 (§5.4)
- [ ] rootfs 빌드 스크립트 및 CI 파이프라인
- [ ] 실제 데이터셋으로 CreateSchema → AlleleCall 완주
- [ ] perUser 인스톨러에서 권한 상승 헬퍼로 `wsl --install --no-distribution` 기동 (§7.5)
- [ ] 가상화 비활성화 기기에서 하드웨어 게이트가 재부팅 이전에 차단하는지 (§7.3 ①)

---

## 부록: 참고 링크

- chewBBACA 공식 문서 — https://chewbbaca.readthedocs.io
- chewBBACA GitHub — https://github.com/B-UMMI/chewBBACA
- Chewie-NS (스키마 저장소) — https://chewbbaca.online
- Bioconda 플랫폼 지원 현황 — https://bioconda.github.io
- Tauri 2 문서 — https://tauri.app
