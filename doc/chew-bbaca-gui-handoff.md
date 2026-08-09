# chewBBACA Desktop GUI — 개발 핸드오프 문서

**대상 플랫폼:** Windows 전용 (macOS 미지원, 향후 검토)
**백엔드 실행 방식:** WSL2 전용 배포판 (Docker 미사용)
**배포 형태:** 오픈소스 공개 + 소수 지인 대상 배포
**작성일:** 2026-08-09

---

## 1. 프로젝트 개요

### 1.1 목적

chewBBACA는 세균의 cg/wgMLST(core/whole genome MultiLocus Sequence Typing) 스키마를 생성하고 allele calling을 수행하는 CLI 전용 소프트웨어다. 터미널 사용에 익숙하지 않은 연구자가 GUI로 동일한 분석을 수행할 수 있게 하는 것이 이 프로젝트의 목표다.

### 1.2 대상 사용자

- 미생물학·감염병 역학 연구자
- 터미널 사용 경험이 거의 없음
- Windows 데스크톱/노트북 사용
- 분석 대상: 수십~수백 개의 세균 유전체 어셈블리(FASTA)

### 1.3 범위에서 제외되는 것

- macOS / Linux 지원 (단, 코드 구조는 확장 가능하게 유지)
- Docker 기반 실행
- 클러스터/HPC 연동
- Chewie-NS 스키마 업로드(`LoadSchema`) 등 쓰기 계열 원격 기능 (초기 릴리스 제외)

---

## 2. 핵심 제약과 그로 인한 아키텍처 결정

### 2.1 chewBBACA는 Windows 네이티브 실행이 불가능하다

이 프로젝트의 모든 복잡도는 여기서 나온다.

- chewBBACA 패키지 자체는 순수 Python(noarch)이라 `pip install chewbbaca`는 Windows에서도 된다.
- 그러나 **Bioconda는 Linux(x86_64, AArch64)와 macOS(x86_64, ARM64)만 지원**한다. Windows 빌드가 존재하지 않는다.
- 외부 의존 바이너리인 **BLAST+, MAFFT, FastTree**를 Windows에서 개별 조달해야 하는데, BLAST+는 Windows 설치판이 있으나 MAFFT/FastTree는 조달과 경로 처리가 까다롭고, chewBBACA 내부의 프로세스 호출·경로 처리에서 Windows 미검증 경로로 인한 버그가 발생하기 쉽다.

**결정: Windows에서는 WSL2 위에 Linux 환경을 구성하고, 그 안에서 chewBBACA를 실행한다.**

### 2.2 왜 Docker가 아닌 WSL 직접 방식인가

| 항목 | WSL 직접 (채택) | Docker (미채택) |
| --- | --- | --- |
| 사용자 사전 설치 | WSL만 | Docker Desktop 추가 필요 |
| 성능 | 레이어 1개 | WSL2 위에 컨테이너 레이어 추가 |
| 프로세스 취소/복구 | 직접 구현 필요 | `docker stop`으로 간단 |
| 라이선스 | 없음 | Docker Desktop 기업 사용 시 유료 |

사용자가 소수이고 "설치하면 바로 되는" 경험이 최우선이므로, 설치 단계를 하나 줄이는 WSL 직접 방식을 채택한다. 프로세스 제어 관련 복잡도(§5.3)는 직접 구현으로 감수한다.

> **참고:** Docker Desktop도 내부적으로 WSL2 백엔드를 사용한다. 즉 Docker를 택했어도 WSL과 가상화 요구사항은 동일하게 발생했을 것이다. 이 선택으로 인해 추가된 사용자 부담은 없다.

---

## 3. 기술 스택

### 3.1 프론트엔드 / 셸

**Tauri 2 + React + TypeScript**

선정 이유:

- **번들 크기**: 10MB 내외. Electron(150MB~) 대비 유리하다. rootfs 자체가 이미 무거우므로 셸은 가벼울수록 좋다.
- **인스톨러/언인스톨러**: `tauri-bundler`가 NSIS(.exe)와 MSI(WiX)를 설정 하나로 생성한다. 언인스톨러도 자동 생성된다. 요구사항에 직접 부합한다.
- **자동 업데이트**: `tauri-plugin-updater` 내장. 서명 키만 있으면 되며 코드 서명 인증서와는 무관하다.
- **프로세스 스트리밍**: Rust 백엔드에서 자식 프로세스 stdout을 이벤트로 프론트에 전달하는 구조가 자연스럽다. AlleleCall이 수십 분 실행되므로 실시간 로그 표시가 필수적이다.
- **HTML 리포트 렌더링**: SchemaEvaluator / AlleleCallEvaluator 산출물이 인터랙티브 HTML이므로 앱 내 WebView에 그대로 표시 가능하다.

### 3.2 검토 후 탈락한 대안

- **Electron**: 자료가 많고 electron-builder도 NSIS/MSI를 잘 만들지만, 용량과 메모리 사용량이 불리하다.
- **PyQt/Tkinter + PyInstaller**: Python GUI가 Python CLI를 호출하는 구조가 직관적으로 보이나, 번들된 앱이 외부 환경을 관리할 때 경로 충돌이 잦다.
- **.NET MAUI / WPF**: Windows 단독으로는 좋으나 향후 macOS 확장을 막는다.

### 3.3 상태 저장

- **SQLite** (`tauri-plugin-sql` 또는 rusqlite)
- 저장 대상: 작업(job) 메타데이터, 실행 이력, 사용자 설정, 스키마 목록
- 앱 재시작 후 진행 중이던 작업을 복구하기 위해 필수 (§5.3 참조)

---

## 4. WSL 환경 설계

### 4.1 전용 배포판을 별도 등록한다

**사용자의 기존 WSL 배포판(Ubuntu 등)을 절대 건드리지 않는다.** 전용 배포판을 `wsl --import`로 등록한다.

```powershell
wsl --import chewie-env "%LOCALAPPDATA%\ChewieApp\wsl" rootfs.tar --version 2
```

장점:

- 사용자의 기존 도구·환경과 버전 충돌이 원천 차단된다.
- 언인스톨이 `wsl --unregister chewie-env` 한 줄로 완결된다. **이 점이 가장 크다.**
- rootfs 이미지를 통째로 교체하는 방식으로 백엔드 업데이트가 가능하다.

설치 위치는 `%LOCALAPPDATA%` 하위를 권장한다. `C:\ProgramData`나 `Program Files`는 관리자 권한이 필요해 설치 경험을 해친다.

### 4.2 rootfs 빌드 파이프라인

CI(GitHub Actions)에서 자동 빌드하여 릴리스에 첨부한다.

```dockerfile
# rootfs 생성 전용 — 런타임에 Docker를 쓰는 것이 아님
FROM mambaorg/micromamba:1.5-jammy
USER root
RUN micromamba install -y -n base -c conda-forge -c bioconda \
      chewbbaca=3.5.4 && \
    micromamba clean -a -y
```

```bash
# 이미지에서 rootfs tar 추출
docker build -t chewie-rootfs .
docker create --name tmp chewie-rootfs
docker export tmp | gzip > chewie-rootfs-3.5.4.tar.gz
docker rm tmp
```

> Docker는 **빌드 타임에만** 사용한다. 사용자 PC에는 Docker가 전혀 필요하지 않다.

체크 항목:

- `/etc/wsl.conf`에 기본 사용자와 `[interop]` 설정을 미리 넣어둔다.
- micromamba 환경이 non-login shell에서도 활성화되도록 `.bashrc` 또는 wrapper 스크립트를 준비한다.
- rootfs 파일에 **SHA256 체크섬을 함께 배포**하고, 앱이 다운로드 후 검증한다.
- 예상 크기: 압축 시 400MB~800MB 수준.

### 4.3 배포 전략

인스톨러에는 GUI만 포함하고, rootfs는 **첫 실행 시 다운로드**한다.

- 인스톨러 크기를 수십 MB로 유지할 수 있다.
- 다운로드 진행률 표시, 재시도, 체크섬 검증 로직이 필요하다.
- GitHub Releases를 호스팅으로 사용한다.

---

## 5. 구현 시 반드시 처리해야 할 항목

### 5.1 파일시스템 성능 — 최우선 고려사항

WSL2에서 `/mnt/c` 접근은 9p 프로토콜을 경유하며 **네이티브 ext4 대비 5~20배 느리다.** chewBBACA는 수백 개 FASTA를 읽고 loci FASTA 수천 개를 쓰므로 이 차이가 치명적이다.

**필수 전략:**

1. 사용자 입력 파일을 WSL 내부 작업 디렉터리(`~/work/{job_id}/input`)로 복사한다.
2. 모든 실행은 WSL 내부 경로에서 수행한다.
3. 결과 중 사용자가 필요로 하는 파일만 Windows로 회수한다.

복사 오버헤드가 있더라도 전체 실행 시간은 크게 단축된다.

**스키마는 WSL 내부에 상주시키고 앱이 관리한다.** 스키마 디렉터리는 allele calling 과정에서 지속적으로 갱신되므로(신규 allele 추가) Windows 측에 두면 성능 손실이 누적된다. GUI에서 스키마 목록을 보여주고 내보내기(export) 기능을 제공하는 형태로 설계한다.

### 5.2 경로 변환

- **`wslpath -a`에 위임한다.** `C:\` → `/mnt/c/` 문자열 치환을 직접 구현하지 않는다.
- 초기부터 테스트해야 할 케이스:
  - 한글이 포함된 경로
  - 공백이 포함된 경로
  - OneDrive 동기화 폴더 (`C:\Users\xxx\OneDrive\...`)
  - UNC 경로 / 네트워크 드라이브 (미지원으로 명시하고 차단할 것)

### 5.3 프로세스 종료 — 좀비 프로세스 문제

**`wsl.exe` 자식 프로세스를 kill해도 내부 Linux 프로세스는 살아남는다.** BLAST가 CPU를 점유한 채 남아 사용자 시스템을 마비시킬 수 있다.

**구현 방침:**

1. 실행 시 `setsid`로 새 프로세스 그룹을 만든다.
2. 그룹 PID를 WSL 내부 파일과 SQLite에 함께 기록한다.
3. 취소 시 별도 `wsl.exe -d chewie-env kill -- -{PGID}` 호출로 그룹 전체를 종료한다.
4. **앱 시작 시 고아 프로세스 청소 로직을 실행한다.** 기록된 PID가 살아 있는지 확인하고, 사용자에게 "이전 작업이 실행 중입니다 — 복구 / 종료"를 선택하게 한다.

### 5.4 실행 환경 세부 설정

| 항목 | 설정 | 이유 |
| --- | --- | --- |
| 환경변수 | `WSL_UTF8=1` | 미설정 시 `wsl --list` 등이 UTF-16LE를 출력해 파싱이 깨진다 |
| 프로세스 플래그 | `CREATE_NO_WINDOW` (`0x08000000`) | Rust `Command`에 `creation_flags`로 지정. 미설정 시 검은 콘솔 창이 반복 노출된다 |
| 환경변수 | `PYTHONUNBUFFERED=1` | tty가 아니어서 출력이 버퍼링된다. 실시간 진행률 표시에 필수 |
| CPU 개수 | WSL 내부 `nproc` 기준 | Windows 논리 코어 수와 다를 수 있다. `--cpu` 인자에 이 값을 사용 |

### 5.5 작업 상태 관리

40분 이상 실행되는 작업이 존재하므로, 창을 닫거나 앱이 종료되어도 상태가 보존되어야 한다.

SQLite에 저장할 항목:

- job id, 모듈명, 전체 인자, 시작/종료 시각
- 상태 (queued / running / completed / failed / cancelled)
- PGID, WSL 작업 디렉터리 경로
- 로그 파일 경로 (로그는 파일로 남기고 DB에는 경로만)
- 출력물 경로

### 5.6 디스크 공간 관리

**`ext4.vhdx`는 파일을 삭제해도 자동으로 축소되지 않는다.** 대용량 분석 후 Windows 디스크 여유 공간이 회복되지 않아 문의가 발생한다.

- 설정 화면에 **"디스크 정리"** 버튼을 제공한다.
- `wsl --manage chewie-env --set-sparse true` 또는 `diskpart`의 `compact vdisk`를 호출한다.
- 실행 전 WSL 종료(`wsl --terminate chewie-env`)가 필요하다.

**`.wslconfig`는 앱이 임의로 수정하지 않는다.** 전역 설정이므로 사용자의 다른 배포판에 영향을 준다. 메모리 제한이 필요하면 안내만 하고 직접 쓰기는 하지 않는다.

---

## 6. 첫 실행 온보딩 (사전 요구사항 검사)

가장 많은 이탈이 발생하는 구간이다. 단계별 검사 후 실패 지점에 맞는 안내를 제공한다.

### 6.1 검사 순서

```
1. 하이퍼바이저 동작 여부
   (Get-CimInstance Win32_ComputerSystem).HypervisorPresent
   → true면 2번은 건너뛴다

2. 펌웨어 가상화 활성화 여부
   (Get-CimInstance Win32_Processor).VirtualizationFirmwareEnabled
   → false면 BIOS 안내 (§6.2)

3. WSL 설치 및 버전
   wsl --status
   → 미설치 / WSL1이면 안내 (§6.3)

4. chewie-env 배포판 존재 여부
   wsl -d chewie-env -- true
   → 없으면 rootfs 다운로드 및 import
```

> **권장 구현:** 위 검사를 순차 실행하기보다, 4번(실제 실행)을 먼저 시도하고 실패했을 때만 원인 진단으로 내려가는 방식이 낫다. 정상 환경 사용자에게 아무것도 묻지 않는 것이 최선이다.

### 6.2 BIOS 가상화 활성화 안내

**Windows 11이라고 해서 가상화가 켜져 있다고 가정하면 안 된다.** Windows 11의 공식 최소 요구사항(TPM 2.0, Secure Boot, 지원 CPU)에 가상화는 포함되지 않는다. VBS(가상화 기반 보안) 기본 활성화 정책 때문에 OEM 완제품은 대체로 켜져 있으나, 다음 경우는 꺼져 있을 수 있다:

- 자가 조립 PC (메인보드 기본값이 Disabled)
- Windows 10에서 업그레이드한 기기
- 안티치트/성능 이슈로 사용자가 의도적으로 끈 경우
- 기업 지급 장비 (IT 정책, BIOS 암호)

**앱이 제공할 것:**

1. **펌웨어 직접 진입 버튼** — 관리자 권한으로 `shutdown /r /fw /t 1` 실행. 재부팅과 동시에 UEFI 설정으로 진입하므로 "부팅 중 F2 연타" 장벽이 사라진다. (UEFI 기기 한정, 레거시 BIOS는 불가)
2. **제조사별 맞춤 안내** — `(Get-CimInstance Win32_ComputerSystem).Manufacturer`로 제조사를 읽어 해당 기종의 진입 키와 메뉴 경로를 표시한다. 설정 항목명은 제조사마다 다르다:
   - Intel: `Intel Virtualization Technology`, `VT-x`, `Vanderpool`
   - AMD: `SVM Mode`
   - 위치: Advanced / CPU Configuration / M.I.T. / OC Tweaker 등
3. **자가 확인 방법 안내** — 작업 관리자 → 성능 → CPU → "가상화: 사용" 표시 스크린샷

### 6.3 WSL 설치 안내

`wsl --install`은 **관리자 권한과 재부팅**을 요구한다.

- 앱이 대신 실행하려 하지 말고, **명령어를 복사 버튼과 함께 안내**한다.
- 관리자 PowerShell 실행 방법을 스크린샷으로 함께 제공한다.
- WSL1으로 설정된 경우 `wsl --set-default-version 2` 안내를 추가한다.

### 6.4 실패 시 대안 제공

BIOS에 관리자 암호가 걸린 회사 장비 등, 끝내 불가능한 사용자가 반드시 발생한다. "실행할 수 없습니다"로 끝내지 말고 대안을 제시한다:

- **Galaxy 웹 버전 안내** — usegalaxy.eu에 chewBBACA 모듈(CreateSchema, AlleleCall, DownloadSchema, PrepExternalSchema)이 등록되어 있어 브라우저에서 실행 가능하다. 단, 버전이 최신보다 뒤처질 수 있고 DownloadSchema에서 균종 매핑 오류가 보고된 이력이 있다.
- **결과 뷰어 모드** — 다른 PC에서 실행한 결과 파일(HTML 리포트, TSV)을 이 앱으로 열어보는 기능. 분석 없이도 앱의 가치를 일부 제공한다.

---

## 7. 기능 범위

### 7.1 chewBBACA 워크플로우 (참고)

```
1. CreateSchema — 어셈블리로부터 wgMLST 스키마 생성
2. AlleleCall — 균주별 allelic profile 결정, 신규 allele을 스키마에 추가
3. ExtractCgMLST — 결과로부터 core genome loci 집합 확정
4. SchemaEvaluator / AlleleCallEvaluator — 인터랙티브 HTML 리포트 생성
```

### 7.2 릴리스별 범위 제안

**v0.1 (MVP)**

- 온보딩 / 환경 구성
- `CreateSchema`, `AlleleCall` 실행
- 실시간 로그 및 진행 표시, 작업 취소
- 결과 폴더 열기 / 내보내기

**v0.2**

- `ExtractCgMLST`, `RemoveGenes`, `JoinProfiles`
- `SchemaEvaluator` / `AlleleCallEvaluator` 리포트 내장 뷰어
- 스키마 관리 화면 (목록, 삭제, 내보내기)

**v0.3**

- `DownloadSchema` (Chewie-NS에서 스키마 내려받기)
- `PrepExternalSchema` (PubMLST/EnteroBase/Ridom 스키마 변환)
- `UniprotFinder` 주석

### 7.3 주요 CLI 매핑 (참조용)

```bash
# 스키마 생성
chewBBACA.py CreateSchema -i <어셈블리폴더> -o <스키마폴더> \
  --ptf <종.trn> --cpu <N>
# CDS 입력 시 --cds 추가

# Allele calling
chewBBACA.py AlleleCall -i <어셈블리폴더> -g <스키마폴더>/schema_seed \
  -o <결과폴더> --cpu <N>
# 일부 loci만: --gl <loci목록.txt>

# core genome 추출
chewBBACA.py ExtractCgMLST -i <results_alleles.tsv> -o <출력> --t 0.95
```

주의:

- Prodigal training file(`.trn`)은 스키마 디렉터리에 저장되어 자동 인식된다. AlleleCall 시 `--ptf`를 다시 넘길 필요가 없으며, **일관된 결과를 위해 동일 training file을 계속 사용해야 한다.**
- chewBBACA v2로 만든 스키마는 `PrepExternalSchema`로 변환이 필요하다.

---

## 8. 배포 및 라이선스

### 8.1 코드 서명

**초기 릴리스에서는 생략한다.** 오픈소스 + 지인 대상이므로 EV 인증서(연 수십만 원) 비용을 감수할 필요가 없다.

- 첫 설치 시 SmartScreen 경고가 표시된다. README에 **"추가 정보 → 실행" 클릭 스크린샷**을 반드시 포함한다.
- Tauri updater의 서명 키는 코드 서명 인증서와 무관하므로, **자동 업데이트는 정상 동작한다.**

### 8.2 배포 채널

- GitHub Releases (인스톨러 + rootfs + 체크섬)
- CI에서 태그 푸시 시 자동 빌드 및 업로드

### 8.3 라이선스

- **chewBBACA는 GPLv3**이다.
- 본 프로젝트는 chewBBACA를 별도 프로세스로 호출하는 래퍼이며, 자체도 오픈소스로 공개하므로 라이선스 충돌 소지가 없다.
- rootfs에 GPL 소프트웨어를 포함해 배포하므로, 소스 취득 경로(각 패키지의 upstream 링크)를 문서에 명시한다.

### 8.4 인용 안내

앱 내 About 화면 및 README에 인용 정보를 표기한다.

> Mamede R, Vila-Cerqueira P, Carriço JA, Ramirez M. 2026. chewBBACA 3: lowering the barrier for scalable and detailed whole- and core-genome multilocus sequence typing. Genome Med 18:51.

---

## 9. 향후 macOS 확장 대비

현재 범위는 Windows 전용이나, 다음 구조만 지켜두면 추후 확장 비용이 크게 줄어든다.

**백엔드 호출부를 trait로 추상화한다:**

```rust
trait ChewieRunner {
    fn ensure_ready(&self) -> Result<()>;
    fn run(&self, module: Module, args: Args) -> Result<JobHandle>;
    fn cancel(&self, handle: &JobHandle) -> Result<()>;
    fn to_backend_path(&self, host: &Path) -> Result<String>;
}
```

- 현재: `WslRunner` 하나만 구현
- macOS 추가 시: `NativeRunner`(micromamba 직접 설치) 구현만 추가하면 된다. macOS는 Bioconda 네이티브 지원이 되므로 WSL 관련 복잡도(경로 변환, 프로세스 그룹, vhdx 관리)가 통째로 사라져 오히려 단순하다.

UI 레이어는 이 trait 뒤에서만 동작하도록 하고, WSL 특화 로직(`wslpath`, `wsl --import` 등)이 프론트엔드로 새어나가지 않게 한다.

---

## 10. 첫 스프린트 권장 작업

GUI보다 먼저 **"Windows에서 chewBBACA AlleleCall을 프로그램적으로 안정 실행"** 프로토타입을 완성한다. 여기서 나머지 설계가 검증된다.

- [ ] rootfs 빌드 스크립트 및 CI 파이프라인
- [ ] `wsl --import` / `--unregister` 라이프사이클 검증
- [ ] Rust에서 프로세스 실행 + stdout 스트리밍 + 그룹 종료 검증
- [ ] `/mnt/c` vs ext4 실행 시간 실측 비교 (§5.1 가설 검증)
- [ ] 한글/공백 경로 테스트
- [ ] 실제 데이터셋으로 CreateSchema → AlleleCall 완주

---

## 부록: 참고 링크

- chewBBACA 공식 문서: https://chewbbaca.readthedocs.io
- chewBBACA GitHub: https://github.com/B-UMMI/chewBBACA
- Chewie-NS (스키마 저장소): https://chewbbaca.online
- Bioconda 플랫폼 지원 현황: https://bioconda.github.io
- Tauri 2 문서: https://tauri.app

