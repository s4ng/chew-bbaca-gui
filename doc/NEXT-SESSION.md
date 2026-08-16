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
| Rust 빌드 | `cargo build --manifest-path src-tauri/Cargo.toml` | 통과 (경고 7, 전부 dead-code) |
| Rust 테스트 | `cargo test --manifest-path src-tauri/Cargo.toml` | **101/101 통과** (v0.4.0) |
| 앱 기동 | `npm run tauri:dev` | 창 생성 확인 (`MainWindowTitle: chewBBACA Desktop`) |
| DB 초기화 | — | `%LOCALAPPDATA%\ChewieApp\app.db` 에 `jobs`/`schemas`/`settings` 생성 확인 (WAL) |
| rootfs 빌드 | `./rootfs/build.sh 3.5.4` | `dist-rootfs/` 에 503MB tar.gz + sha256 (2026-08-10) |
| 인스톨러 | `npm run tauri:build` | 509.7MB NSIS 생성. rootfs 502.7MB 가 그대로 들어 있다 |
| 배포판 등록 | `wsl --import chewie-env ...` | **8초**. `chewie-env` 등록됨 (2026-08-10) |
| micromamba 활성화 | `wsl -d chewie-env -- bash -lc 'chewBBACA.py --version'` | `chewBBACA version: 3.5.4` |
| PGID 회수 | `runner/wsl.rs` 의 스크립트를 그대로 실행 | `__CHEWIE_PGID__ 20`, 자식 2개 모두 PGID 20 |
| 취소 | 같은 PGID 에 `kill -TERM -20` | 잔존 프로세스 **0**, 러너 프로세스 exit 143(SIGTERM) |
| 경로 변환 | `wslpath -a 'C:\...\한글 경로 (괄호)'` | 한글·공백·괄호 모두 통과 |
| **CreateSchema 완주** | `chewBBACA.py CreateSchema --cds` | 2초, **20 loci** 생성. `schema_seed/` 구조 확인 |
| **AlleleCall 완주** | `chewBBACA.py AlleleCall --cds` | 2초, EXC 20 / INF 60. `results_alleles.tsv` 생성 |
| 진행률 파싱 | `runner/progress.rs` | 위 두 로그로 **교정 완료**. 테스트가 실측 로그를 재생한다 |
| **SchemaEvaluator 완주** | 앱에서 실행 (loci 3,127) | 2초. `schema_report.html` 회수 (2026-08-11) |
| **AlleleCallEvaluator 완주** | 앱에서 실행 (균주 32 × loci 3,127) | 38초. `allelecall_report.html` 회수 (2026-08-11) |
| 한글·괄호 결과 경로 | `…\리포트 테스트 (평가)\se` | 스테이징·회수 모두 통과 |
| **RemoveGenes 완주** | 앱에서 실행 | loci 3,127 − 목록 1,270 = **1,857**. 목록 loci 잔존 0 (2026-08-11) |
| **JoinProfiles 완주** | 앱에서 실행 | 3,127×32 + 1,270×32 `--common` → **1,270 loci × 64행** (2026-08-11) |
| **MCP 서버** | 개발 실행에 HTTP 로 직접 접속 | 도구 17개 노출, 인증·Origin·바인드 게이트 통과 (2026-08-12) |
| **MCP 실제 실행** | `chewie_create_schema` (게놈 32개) | **49초에 completed**, loci 3,130 스키마 등록 (2026-08-12) |
| **ChatGPT Desktop 실접속** | 앱에서 등록 → Work 모드 대화 | **정상 동작.** 요청 24건 거절 0건 (2026-08-12) |
| MCP 규격 적합성 | `@modelcontextprotocol/inspector --cli` | 도구 17개 목록 + `tools/call` 실행 확인 |
| **pyrodigal 학습 배관** | 합성 게놈으로 `pyrodigal -p single -t` 직접 실행 | `.trn` **558KB** 생성. `-t` 재사용 함정 실측 (2026-08-14) |

> 완주는 **합성 CDS 입력**(게놈 4개 × 유전자 20개, `--cds` 로 Prodigal 우회)으로 했다.
> 단계 이름과 순서를 얻는 데는 충분하지만, 단계별 **비중**은 실제 어셈블리로 다시 봐야 한다.
> 진짜 데이터에서는 BLASTp 두 구간이 지금 배분보다 훨씬 무거울 것이다.

> **빌드는 반드시 PowerShell 에서 돌린다.** Git Bash 의 PATH 는 `link` 를
> `/usr/bin/link`(coreutils)로 잡아 MSVC 링커를 가린다. 증상이
> `could not compile <crate> (build script)` 로만 나와 코드 문제로 오해하기 쉽다.

### 검증되지 **않은** 것

2026-08-09 판의 이 목록(인스톨러·`JobManager` 전체 경로·스테이징·스키마 등록·조정)은
**8/10~8/11 에 전부 해소됐다.** 인스톨러로 설치한 앱에서 파이프라인을 완주시켰고,
언인스톨 훅도 체크박스 양쪽으로 확인했다(④).

**MCP 서버(v0.3.0)에서 하나가 남았다 — ChatGPT Desktop 으로 실제 접속.** 프로토콜은
curl 로만 확인했다. 자세한 것은 [`MCP.md`](MCP.md) §9~§10.

> ### ⚠ v0.4.0 training file 기능 — 실물 검증이 남아 있다
>
> 배관(pyrodigal 호출·`.trn` 생성·`-t` 재사용 함정)은 **합성 게놈으로** 확인했다.
> 확인하지 **않은** 것 셋:
>
> 1. **실제 폴더에서 선별이 납득할 만한가.** `fasta.rs` 의 3단계 판정(100kb 미만 제거 →
>    중앙값 ±20% → contig 최소)은 단위 테스트로만 검증됐다. 실측 데이터가 바로 옆에 있다 —
>    `C:\Users\zalcl\chewBBACA_tutorial\...\complete_genomes\` 의 *S. agalactiae* 32개.
> 2. **32개(또는 수백 개)를 훑는 데 걸리는 시간.** 폴더의 FASTA 를 **전부 읽는다**
>    (contig 수는 파일 크기로 알 수 없다). UI 는 그동안 "훑는 중…" 만 띄운다.
> 3. **만든 `.trn` 으로 CreateSchema 가 완주하는가.** 이게 통과해야 기능이 의미를 갖는다.
>
> 다음 세션의 첫 작업으로 권한다. 셋 다 앱에서 클릭 몇 번이면 끝난다
> ([새 작업] → CreateSchema → training file 칸 → [폴더에서 만들기]).

**여덟 모듈 모두 앱을 거쳐 완주했다** (2026-08-11). 남은 것은 셋뿐이다.

- **[리포트 열기] 버튼을 거치는 경로** — 리포트 자체는 사용자가 확인했지만, 그것이
  버튼을 통한 것인지 폴더에서 직접 연 것인지는 구분되지 않았다. 아래 ⑤-2 각주.
- `/mnt/c` vs ext4 실행 시간 차이 (§5.2 의 전제 자체) — 아래 ②
- **다른 PC 에서의 설치** — 이 PC 에서만 설치·제거를 확인했다. 아래 ⑤ 참조.

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
- **`chewie-env` 배포판이 2026-08-10 에 등록되었다** (수동 `wsl --import`). 앱을 켜면
  온보딩을 통과해 바로 진입한다. 지우려면 `wsl --unregister chewie-env`.
  온보딩 ③ 을 다시 보려면 지운 뒤 켜면 된다 — 개발 실행에는 동봉 rootfs 가 없으므로
  설치 버튼 대신 안내가 뜨는 것이 정상이다 (§3-① 참조).
- 배포판 안의 `/tmp` 는 비어 있다 (2026-08-11 에 검증용 잔재를 정리했다).
  실데이터는 Windows 쪽 `C:\Users\zalcl\chewBBACA_tutorial\` 에 있다 —
  *S. agalactiae* 완성 게놈 32개(`genomes\complete_genomes\complete_genomes\*.fna`),
  loci 3,127개 스키마, AlleleCall 결과 폴더 셋, `cgMLSTschema95.txt`(loci 목록 1,270).
  모듈을 검증할 때 합성 데이터를 새로 만들 필요가 없다.
- `%LOCALAPPDATA%\ChewieApp\` 은 스모크 테스트로 이미 생성되어 있다 (정상, 앱이 재사용한다).
- git 저장소로 초기화되어 있다 (`main` 브랜치, 초기 커밋 `dad4469`).
  원격은 <https://github.com/s4ng/chew-bbaca-gui> (**공개**). `gh` 는 `s4ng` 로 인증돼 있다.
  릴리스는 **태그에 `v` 를 붙이지 않고**(`0.2.0`) 제목에만 붙인다(`v0.2.0`) — 기존 관례다.
  인스톨러는 릴리스 자산으로 직접 올린다 (510MB, GitHub 자산 한도 2GiB 안).
  `.gitattributes` 가 `rootfs/*.sh` 를 LF 로 고정한다 — **이 설정을 풀면 Windows 체크아웃에서
  CRLF 로 바뀌어 rootfs 이미지 안의 `/etc/profile.d/chewie.sh` 가 깨진다.**

---

## 3. 다음 작업 (우선순위 순)

### ① rootfs 빌드와 배포판 등록 — ✅ 2026-08-10 완료

이미지는 `dist-rootfs/chewie-rootfs-3.5.4.tar.gz` (503MB, sha256 `9d1cb6e0…`) 로 있고,
등록·실행·취소·스키마 등록까지 앱을 거쳐 확인됐다. 이미지를 고칠 때만 다시 빌드한다.

```bash
# Linux 또는 WSL(Ubuntu) 안에서, Docker 필요
./rootfs/build.sh 3.5.4          # → dist-rootfs/chewie-rootfs-3.5.4.tar.gz + .sha256
```

배포판을 다시 등록해야 할 때 쓸 수 있는 세 가지 (개발 편의 순서로는 두 번째가 빠르다).

- **인스톨러로:** rootfs 가 동봉된 NSIS 설치 → 첫 실행에서 온보딩 ③ 의 [설치] 버튼.
  **실제 배포 경로와 같으므로 최종 확인은 반드시 이쪽으로 한다.**
- **개발 실행에서:** `tauri:dev` 에는 동봉본이 없다. `설정 → rootfs 이미지` 칸에
  tar.gz 의 **절대 경로**를 넣으면 검증 후 등록한다.
- **수동으로:** `wsl --import chewie-env "%LOCALAPPDATA%\ChewieApp\wsl" <tar.gz> --version 2`

> **`bash -lc` 를 거치지 않으면 안 된다.** micromamba 활성화가 프로필에서 일어나므로
> `wsl -d chewie-env -e python3` 처럼 직접 실행하면 `/opt/conda/bin` 이 PATH 에 없어
> 실패한다. `exec()` 로 부르는 것은 coreutils(`printenv`, `cp` 등)로 한정한다.

### ② `/mnt/c` vs ext4 실측 (§5.2 의 전제)

같은 데이터셋으로 두 번 돌려 시간을 잰다. 지금 구조는 "입력을 복사하는 비용보다
9p 오버헤드가 훨씬 크다" 는 가정 위에 서 있다. 만약 차이가 작다면 스테이징 단계
(`WslRunner::stage_input`)를 선택적으로 만들 여지가 생긴다.

### ③ 진행률 파싱 교정 — ✅ 2026-08-10 완료

실측 로그로 `progress.rs` 를 다시 썼다. 바뀐 점:

- 단계표가 **모듈별로 분리**되었다 (`ProgressParser::for_module`). 두 모듈이
  `CDS deduplication` 같은 이름을 공유하지만 순서와 비중이 다르다.
- 옛 키워드 5개 중 **3개는 로그에 존재하지도 않았다**
  (`extracting cds`, `removing duplicated`, `reading the input files`).
- 막대(`[====] 40%`)가 붙지 않는 단계가 많아, **단계 헤더만으로도 구간 시작까지 올린다.**
- 테스트가 실측 로그 줄을 그대로 재생한다 (`CREATE_SCHEMA_LOG`, `ALLELE_CALL_LOG`).

**비중도 2026-08-11 에 실제 어셈블리로 다시 쟀다** (완성 게놈 32개, `--cpu 8`).
합성 데이터(2초)로 잡았던 옛 비중은 실제와 크게 달랐다.

| 모듈 | 총 시간 | 가장 무거운 단계 | 나머지 전부 |
| --- | --- | --- | --- |
| CreateSchema | 38초 | 클러스터별 BLASTp **73%** | 27% (최종 BLASTp 12% 포함) |
| AlleleCall | 1분 30초 | 대표 서열 정렬 **77%** | 23% (분류 7%, self-score 5%) |

한 단계가 압도한다는 것이 요지다. 옛 표처럼 앞쪽 단계에 넉넉히 배분하면 막대가
초반에 훌쩍 올라갔다가 정작 오래 걸리는 구간에서 멈춘 것처럼 보인다.

> **표에 단계가 하나 통째로 빠져 있었다.** 어셈블리를 입력하면 Prodigal 이 돌아
> `Predicting CDSs for N inputs...` 가 나오는데, 합성 데이터는 `--cds` 로 넣어
> 그 단계가 아예 없었다. 두 모듈 모두 빠져 있었고 지금은 `Renaming CDSs`(--cds 경로)와
> 같은 구간을 나눠 갖는다. **입력 종류를 바꾸면 로그의 단계 자체가 달라진다** —
> 새 단계표를 만들 때는 어느 쪽 입력으로 얻은 로그인지 함께 봐야 한다.

### ④ 조정(reconciliation) 실경로 확인 — ✅ 2026-08-10 완료

인스톨러로 설치한 앱에서 CreateSchema 실행 중 창을 닫고 다시 열어 확인했다.
배너 표시 → 화면을 오가도 유지 → [복구] → 완료 후 스키마가 **이름과 loci 수를 갖고**
등록되는 것까지 통과했다. 언인스톨 훅도 같은 날 확인했다 —
체크박스를 켠 제거 후 `chewie-env`·`%LOCALAPPDATA%\ChewieApp`·설치 폴더가 모두 사라졌다.

여기까지 오는 동안 잡은 결함은 §4 의 함정 목록에 남겼다.

> **2026-08-10 에 오탐 하나를 잡았다.** 프런트는 [작업] 화면을 열 때마다
> `jobs_reconcile` 을 부르는데, 조정이 매번 다시 돌면서 **지금 이 프로세스가 실행 중인
> 작업**을 고아로 오판했다. `mark_running` 직후에는 PGID 표식이 아직 도착하지 않아
> `is_alive` 가 false 를 주기 때문이다. 결과적으로 정상 실행 중인 작업이 잠깐
> "앱이 종료된 사이 프로세스가 사라졌고 결과도 없습니다" 로 표시됐다가, 실제 완료 시
> 다시 `completed` 로 덮여 쓰였다. 이제 조정은 **프로세스당 한 번만** 돌고
> 자기 작업은 건너뛴다.
>
> 아직 남은 문제 — `output_populated()` 는 `~/work/{job}/output` 만 본다. CreateSchema 는
> 산출물이 `~/schemas/{id}` 로 가므로 **진짜 고아가 된 CreateSchema 는 성공했어도
> 실패로 확정된다.** ④ 를 검증할 때 이것부터 고쳐야 한다.

### ⑤-0 PrepExternalSchema 검증 — ✅ 2026-08-11 완료

문서·소스만 보고 구현했던 가정을 실제로 돌려 확인했다. 입력은 내보내둔 스키마에서
loci 12개를 뽑아 만든 폴더다.

- **출력 구조 가정이 맞았다.** 이 모듈은 `-o` 아래에 `schema_seed/` 를 만들지 않고
  loci FASTA 를 **바로** 푼다. 그래서 `-o` 를 `{스키마}/schema_seed` 로 겨눴고,
  앱의 loci 계수(`ls {p}/schema_seed/*.fasta`)가 그대로 12를 돌려준다.
- 부산물(`schema_seed_invalid_loci.txt` 등)은 `-o` 의 **형제**로 떨어진다.
  스키마 폴더 최상위에 놓이므로 CreateSchema 의 `cds_coordinates.tsv` 와 같은 취급이다.
- **들여온 스키마로 AlleleCall 이 정상 완주했다** (EXC 21 / 신규 0).
- 진행률 표를 실측 로그로 교정했다. 추측했던 `adapting schema` 는 로그에 없는
  문구였고 실제로는 `Adapting 12 loci...` 다.

**2026-08-11: 앱 UI 로도 완주 확인.** 폼에서 폴더를 골라 실행 → [스키마] 화면에
loci 수까지 정상 표시. `stage_input` → `-g` 연결도 이로써 검증됐다.

이로써 **네 모듈 모두 UI 경로로 검증 완료**다.

### ⑤-1 평가 리포트 두 개 — ✅ 2026-08-11 완료

UI 노출·실행 검증·진행률 교정·[리포트 열기]까지 끝냈다. **앱을 거쳐 두 모듈 모두
완주했고**(loci 3,127 / 균주 32) 리포트가 한글·공백·괄호가 든 결과 폴더로
회수되는 것까지 확인했다. 실측으로 드러난 것들:

- **리포트 파일명 확정** — `schema_report.html` / `allelecall_report.html`.
  옆에 15MB 안팎의 `report_bundle.js` 가 함께 놓인다. `commands.rs` 의 테스트가
  이 이름을 굳혀 둔다.
- `--loci-reports` 는 loci 마다 MAFFT 를 돌린다. loci 3,127 기준 **3초 → 39초**,
  회수 파일도 2개 → 3,130개가 된다. 폼에 그 사실을 적어두었다.
- 두 모듈 다 산출물의 `temp/` 를 스스로 지운다. 회수 비용을 걱정할 필요가 없다.
- 진행률 비중은 실측대로다. AlleleCallEvaluator 는 MSA(44%)와 NJ 트리(44%)가
  거의 전부이고 앞의 여러 단계는 다 합쳐 2초다.

### ⑤-2 리포트를 브라우저로 여는 경로 — ✅ 2026-08-11 완료

**내장 뷰어는 만들지 않기로 했다.** 앱 웹뷰에 띄우려면 CSP 를 열고 asset 프로토콜을
붙여야 하는데, 리포트는 확대·검색·인쇄가 되는 브라우저에서 보는 편이 낫다.

`commands.rs::report_open` 이 회수된 폴더에서 리포트를 찾아 `opener` 로 연다.
[작업 상세] 는 **완료된 평가 작업**에만 [리포트 열기] 를 띄운다(회수 전에는 열 것이 없다).
여는 방식은 [따라해보기] 와 같다 — 프런트의 `openPath` 는 스코프가 비어 있어
열리지 않으므로 Rust 에서 `app.opener().open_path(...)` 로 연다.

> **버튼 자체를 눌러본 것은 아직 아니다.** 파일을 찾는 부분(`find_report`)은
> 단위 테스트가 지키고, 여는 부분은 [따라해보기] 로 이미 검증된 같은 경로다.

### ⑤-3 RemoveGenes · JoinProfiles 실행 검증 — ✅ 2026-08-11 완료

앱에서 실행해 **의심하던 두 지점이 모두 정상**임을 확인했다.

- **`-o` 가 폴더가 아니라 파일인 경로가 동작한다.** 두 모듈은 산출물이 파일 하나라
  `{work}/output/` 안에 만든 뒤 폴더째 회수하는데, 실제로 그 자리에 떨어져
  결과 폴더로 회수됐다 (`results_alleles_filtered.tsv`, `joined_profiles.tsv`).
- **`stage_files()` 의 이름 바꾸기가 결과로 새지 않는다.** 입력이 하나같이
  `results_alleles.tsv` 라 `0.tsv`/`1.tsv` 로 바꿔 복사하는데, 합쳐진 표의 열 이름과
  균주 이름 어디에도 그 이름이 나타나지 않았다.

숫자로도 맞다 — RemoveGenes 는 `3,127 − 1,270 = 1,857` 이고 제거 대상 loci 의
잔존이 0이다. JoinProfiles 는 `--common` 으로 교집합 1,270 loci, 행은 32+32=64 다.

> **JoinProfiles 를 시험할 때는 균주가 겹치지 않는 결과 두 개를 써야 한다.** 위 검증은
> 같은 균주 32개짜리 결과 둘을 합친 것이라 **모든 균주가 두 번씩** 들어갔다. 배관을
> 확인하는 데는 충분하지만 그 표 자체는 분석에 쓸 수 없다. 제대로 하려면 어셈블리를
> 나눠 AlleleCall 을 두 번 돌린 결과를 합친다.

### ⑥ MCP 서버 — v0.3.0 에서 구현 완료

설계와 실측은 [`MCP.md`](MCP.md) 에 있다. 요지만 적으면:

- 앱이 켜져 있는 동안 `127.0.0.1:8787/mcp` 에서 Streamable HTTP 로 MCP 를 말한다.
  베어러 토큰은 첫 기동에 발급되어 DB 와 `%LOCALAPPDATA%\ChewieApp\mcp.json` 에 남는다.
- 도구 17개(읽기 7 + 실행 8 + 취소·리포트). **되돌릴 수 없는 조작은 노출하지 않는다** —
  스키마 삭제·배포판 제거·재부팅·디스크 정리·설정 변경은 MCP 에 없다.
- 설정 화면에 켬/끔, 포트, [작업 실행 허용], [설정 복사], [토큰 재발급] 이 있다.

- 설정 화면에서 **URL · 헤더 키 · 헤더 값**을 칸마다 복사한다. 등록 화면이 폼이라
  설정 파일 문법(TOML)을 통째로 주면 쓸모가 없다 — TOML 은 접어서 남겼다(Codex CLI 용).
- [연결 방법 보기] 가 그림이 든 안내(`guide/mcp.html`)를 브라우저로 연다.

**ChatGPT Desktop 실접속을 통과했다.** 손으로 짠 최소 Streamable HTTP 구현이 실제
클라이언트를 통과하므로 `rmcp` 로 갈아탈 이유가 없어졌다. 붙이는 과정에서 물린 것 셋:

1. **대화창을 `Work` 모드로 바꿔야 도구가 보인다.** `Chat` 모드에서는 등록이 완벽해도
   모델에게 노출되지 않는다. 증상이 "그런 도구가 없다" 라 설정 문제로 오해하기 쉽다 —
   **여기서 가장 오래 막혔다.**
2. 토큰은 폼의 **[헤더]** 에 넣는다. [기본 token 환경 변수] 칸은 환경 변수의 *이름*을
   받는 자리라 토큰을 그대로 넣으면 401 이 된다.
3. **앱을 두 개 켜면 나중 것이 8788 로 밀린다.** 클라이언트는 8787 에 고정돼 있으므로
   그 순간 연결이 끊긴다. 고치지 않고 남겨둔 문제다 — `MCP.md` §11.

진단은 `%LOCALAPPDATA%\ChewieApp\mcp.log` 로 한다. 기록이 비어 있으면 클라이언트가
닿지도 못한 것이고, `tools/list` 까지 찍혀 있으면 연결은 정상이므로 클라이언트 쪽을 본다.

### ⑤ 그다음 (v0.2 범위)

- ~~`ExtractCgMLST` 추가~~ ✅ **2026-08-10 구현 완료** (UI 경로 검증은 §5-0 참조).
  이 모듈이 기존 두 개와 다른 점 세 가지가 다음 모듈 추가의 참고가 된다.
  - 입력이 폴더가 아니라 **파일 하나**다 → `Module::takes_input_dir()` 로 갈라
    `stage_file()` 이 그 파일만 ext4 로 복사한다. `submit()` 의 `input_dir` 검증도 조건부다.
  - **`--cpu` 인자가 없다.** `build_argv` 가 무조건 붙이던 것을 모듈별로 바꿨다
    (붙이면 argparse 가 즉시 실패한다).
  - 진행률 막대가 없어 단계 표시가 거칠다. 실행이 짧아 문제되지 않는다.
- ~~다음 모듈을 넣기 전에 `JobSpec` 정리~~ ✅ 2026-08-10 에 `ModuleParams` 열거형으로
  갈랐다. 모듈이 넷 더 붙는 동안 이 구조가 버텼다 — 새 모듈은 variant 하나만 더한다.
- ~~리포트 내장 뷰어~~ ❌ **만들지 않기로 했다** (§5-2). 결과 뷰어 모드(§7.7)는 남아 있다.
- **배포 — 이제 여기가 다음 세션의 첫 작업이다.** v0.2 기능 범위는 닫혔다
  (여덟 모듈 전부 앱에서 완주). 남은 것은 내보내는 일이다.
  - 버전을 올린다. **세 곳이 각자 적혀 있다** — `package.json`,
    `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`. 어긋나면 인스톨러 파일명과
    앱 정보가 따로 논다.
  - `npm run tauri:build` → `dist-rootfs/` 의 tar.gz 가 동봉된다. rootfs 파일명은
    **네 곳**에 흩어져 있다 (CLAUDE.md 참조).
  - 인스톨러를 **다른 PC 에서** 설치해 첫 실행까지 확인. 이 PC 는 이미 `chewie-env` 가
    있어 온보딩 ③(rootfs 등록)을 지나가므로, 그 경로는 다른 기기에서만 검증된다.
  - 지인 배포는 zip 전달이므로 SmartScreen 경고 안내(§8.4)를 README 에 넣어야 한다.

---

## 4. 손대기 전에 알아야 할 함정

- **`HypervisorPresent = False` 는 "BIOS 가 꺼져 있다" 가 아니다.** (2026-08-16 실사용자
  신고, v0.4.1 에서 수정) Virtual Machine Platform 이 없는 기기는 BIOS 에서 VT-x 가
  켜져 있어도 하이퍼바이저가 기동조차 하지 않아 `False` 다 — **WSL 을 한 번도 깔지 않은
  기기의 정상 상태다.** 이 값 하나로 게이트 ①을 막으면 BIOS 를 이미 켠 사용자가
  [다시 검사] 를 눌러도 영영 통과하지 못한다(신고 기기: LG gram 14ZD90P, 펌웨어 `True` /
  하이퍼바이저 `False` / WSL 미설치). 판정은 `probe.rs::hardware_verdict()` 한 곳에
  모여 있다 — 펌웨어가 `True` 면 보류하고 ②로 내려간 뒤, **WSL 이 이미 설치됐는데도**
  하이퍼바이저가 없을 때 실패로 확정한다. `scripts/check-env.bat` 은 처음부터 이 세 갈래
  (`PASS` / `PASS-PENDING` / `BIOS-SUSPECT`)로 되어 있었다. 둘이 어긋나면 스크립트 쪽이 맞다.

- **모듈에 따라 `-o` 가 이미 있으면 거부한다.** (2026-08-11 에 실제로 물림)
  `Output directory already exists.` 한 줄과 exit 1 이 전부다 — 로그만 보면 왜
  아무 일도 안 일어났는지 알기 어렵다. 검사를 가진 모듈은 넷이다:
  `CreateSchema` · `PrepExternalSchema`(=adapt_schema) · **`SchemaEvaluator`** ·
  **`AlleleCallEvaluator`**. 앞의 둘은 `-o` 가 아직 없는 스키마 경로라 문제가 없지만,
  평가 두 모듈은 `-o` 가 `{work}/output` 이고 **스테이징이 그 폴더를 미리 만든다.**
  그래서 `wsl.rs::drop_empty_output()` 이 실행 직전에 빈 output 을 도로 지운다
  (`rmdir` 이라 비어 있을 때만 지워진다). 새 모듈을 넣을 때 `-o` 를 `{work}/output`
  으로 겨눈다면 **그 모듈에 이 검사가 있는지부터 확인한다** —
  `grep -n OUTPUT_DIRECTORY_EXISTS chewBBACA.py` 한 줄이면 된다.

- **AlleleCallEvaluator 는 `cds_coordinates.tsv` 를 요구한다.** 그 파일은 AlleleCall 이
  Prodigal 로 CDS 를 예측했을 때만 나온다 — `--cds` 로 돌린 결과 폴더에는 **없다.**
  없으면 모듈이 파이썬 traceback 으로 죽으므로 `jobs.rs::submit()` 에서 미리 막는다.
  (`allele_call.py` 가 `cds_input` 일 때 `cds_coordinates = None` 으로 두고,
  `evaluate_calls.py` 는 그 파일을 조건 없이 연다.)

- **chewBBACA 는 진행 중인 작업을 줄바꿈 없이 찍고 끝난 뒤에 `done.` 을 붙인다.**
  우리 `pump()` 는 `\n`/`\r` 로만 자르므로 **그런 줄은 이미 끝난 뒤에야 도착한다.**
  AlleleCallEvaluator 의 NJ 트리(전체의 44%, 15초)가 그렇다. 진행률 단계를 그 줄에
  걸면 막대가 죽은 것처럼 보인다 — **바로 앞 줄**을 진입 신호로 삼아야 한다
  (`progress.rs` 의 `results are available in` 참조).

- **serde 의 `rename_all` 은 열거형에서 *variant 이름*을 바꾼다.** 필드를 바꾸는 것은
  `rename_all_fields` 다. `ModuleParams` 에 `rename_all = "camelCase"` 만 붙였다가
  태그가 `createSchema` 가 되어 **네 모듈 전부 제출이 불가능**했다 (2026-08-11).
  variant 는 `Module` 과 같은 PascalCase, 필드는 camelCase 여야 한다.
  - 더 일반적인 교훈: **양쪽 타입이 각자 맞아도 그 사이 문자열 표현은 어긋날 수 있다.**
    Rust 도 컴파일되고 `tsc` 도 통과했지만 실제 IPC 는 깨져 있었다.
    `models.rs` 의 테스트가 **프런트가 보내는 JSON 을 문자열 그대로** 넣어 왕복시킨다.
    `JobSpec` 이나 `types.ts` 를 고치면 그 테스트의 JSON 도 함께 고친다.

- **작업의 stdout 을 앱의 파이프로 직접 받으면 안 된다.** (2026-08-10 실측)
  앱이 닫히면 파이프가 닫히고, 거기에 쓰던 chewBBACA 가 SIGPIPE 로 죽는다.
  `setsid` 는 프로세스 그룹만 분리할 뿐 출력 대상까지 떼어주지는 않는다.
  실측: `sleep`(출력 없음)은 살아남고, 1초마다 `echo` 하는 프로세스는 죽는다.
  그래서 `spawn_and_stream()` 은 `run.log` 에 쓰게 하고 `tail -n +1 -f --pid=` 로
  중계한다. `--pid` 를 빼고 직접 `kill` 하면 **마지막 몇 줄(요약 표, "Finished at")을
  놓친다.**

- **`wsl.exe` 에는 `-e` 를 쓴다. `--` 를 쓰면 안 된다.** (2026-08-10 에 실제로 물림)
  `--` 는 명령줄을 배포판 기본 셸에 **한 번 더 파싱**시켜 인용부호를 먹는다. 그래서
  `export CHEWIE_CMD='...'` 가 무력화되고 `eval` 이 빈 문자열을 돌린다 —
  **에러 없이 exit 0 으로 끝나고 아무 일도 일어나지 않는다.** 앱에서 CreateSchema 를
  돌렸는데 로그에 chewBBACA 출력이 한 줄도 없고 스키마도 안 생긴 것이 이 증상이었다.
  `runner/wsl.rs::login_shell_argv()` 로 한곳에 모았고 회귀 테스트가 지킨다.
  확인법: `wsl -d chewie-env -- bash -lc "X='a b'; echo [\$X]"` → `[]` (깨짐),
  같은 것을 `-e` 로 → `[a b]`.

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
  `devUrl`(localhost:5173)을 보기 때문이다. 항상 `npm run tauri:dev` 로 띄운다.
- **`vite dev` 가 `EACCES ::1:<포트>` 로 죽으면 코드 문제가 아니다.** Windows 의
  Hyper-V/WSL2 가 부팅할 때마다 포트 대역을 동적으로 예약하는데, 개발 포트가 거기
  걸리면 점유가 아니라 **권한 거부**로 실패한다 — 포트를 쓰는 프로세스를 찾아도 없다.
  `netsh interface ipv4 show excludedportrange protocol=tcp` 로 확인한다.
  Tauri 기본값 1420 에서 5173 으로 옮긴 것이 이 때문이다(2026-08-14). 5173 도 언젠가
  같은 대역에 걸릴 수 있으므로, 반복되면 관리자 권한으로 영구 예약하는 편이 낫다:
  `net stop winnat` → `netsh int ipv4 add excludedportrange protocol=tcp startport=5173
  numberofports=1 store=persistent` → `net start winnat`.
  **포트를 바꿀 때는 `vite.config.ts` 와 `src-tauri/tauri.conf.json` 의 `devUrl` 을
  함께 고친다.** 한쪽만 고치면 Vite 는 정상 기동하고 앱만 빈 화면을 띄운다.

---

## 5. 하지 말 것

- 사용자의 `Ubuntu` 배포판 수정/삭제, `.wslconfig` 접근
- `wsl --install` 에서 `--no-distribution` 제거
- `runner/` 밖으로 WSL 개념(`wslpath`, PGID, `/mnt/c`) 노출
- `/mnt/c` 위에서 chewBBACA 실행 (측정 목적의 ②는 예외)
- 실측 없이 `progress.rs` 의 숫자를 "그럴듯하게" 조정하는 것 — 그러면 진짜 교정이 늦어진다
