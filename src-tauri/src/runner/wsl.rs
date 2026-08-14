//! `WslRunner` — 현재 유일한 `ChewieRunner` 구현체 (§4.1).
//!
//! 여기서만 알아도 되는 것들:
//! * 경로 변환은 `wslpath -a` 에 위임한다. `C:\` → `/mnt/c/` 문자열 치환을
//!   직접 구현하지 않는다 (OneDrive·매핑 드라이브·한글 경로에서 어긋난다).
//! * 모든 실행은 ext4 내부(`~/work/{job_id}`)에서 한다. `/mnt/c` 위에서
//!   연산하면 9p 오버헤드로 5~20배 느려진다 (§5.2).
//! * `setsid` 로 새 프로세스 그룹을 만들고 PGID 를 즉시 올려보낸다. 이게 없으면
//!   `wsl.exe` 를 죽여도 BLAST 가 코어를 물고 살아남는다 (§6.2).

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;

use crate::error::{Error, Result};
use crate::models::{JobSpec, Module, ModuleParams};
use crate::paths::validate_host_path;
use crate::runner::cli::{build_argv, training_argv, BackendArgs};
use crate::runner::{
    schema_id_for, schema_name_of, BackendStatus, ChewieRunner, CreatedSchema, EventSink,
    JobHandle, RunEvent, RunOutcome,
};
use crate::util::sh_quote;
use crate::win;

/// PGID 를 stdout 으로 실어 보내기 위한 표식. 이 줄은 로그에 노출하지 않는다.
const PGID_MARKER: &str = "__CHEWIE_PGID__";

/// 작업 디렉터리 안의 실행 로그. chewBBACA 는 앱의 파이프가 아니라 **이 파일**에 쓴다.
/// 앱이 죽어도 쓰기 대상이 사라지지 않아야 작업이 살아남는다 (§6.3).
const RUN_LOG: &str = "run.log";

/// `wsl.exe` 에 명령을 넘기는 방식. **`--` 로 바꾸지 마라.**
///
/// `wsl.exe -d <distro> -- <명령>` 은 명령줄을 배포판의 **기본 셸에 한 번 더 파싱**시킨다.
/// 그 재파싱이 우리가 붙인 인용부호를 먹어버려서, 공백·한글이 든 경로가 조각나고
/// `export CHEWIE_CMD='...'` 같은 문장이 통째로 무력화된다.
///
/// 증상이 고약하다 — **실패하지 않고 조용히 아무것도 하지 않는다.** 2026-08-10 에
/// CreateSchema 가 출력 한 줄 없이 exit 0 으로 끝나고 스키마도 안 생기는 것으로 드러났다.
/// `-e` 는 셸을 거치지 않고 곧바로 exec 하므로 argv 경계와 인용이 그대로 보존된다.
const EXEC_FLAG: &str = "-e";

/// 로그인 셸로 스크립트 하나를 실행하는 argv. `-l` 은 micromamba 활성화용이다(§8.2).
fn login_shell_argv(script: &str) -> [&str; 4] {
    [EXEC_FLAG, "bash", "-lc", script]
}

pub struct WslRunner {
    distro: String,
    /// `$HOME` 조회 결과 캐시 (배포판당 한 번이면 충분하다)
    home: Mutex<Option<String>>,
}

impl WslRunner {
    pub fn new(distro: impl Into<String>) -> Self {
        Self {
            distro: distro.into(),
            home: Mutex::new(None),
        }
    }

    pub fn distro(&self) -> &str {
        &self.distro
    }

    /// `wsl.exe -d <distro>` 기본 명령. 콘솔 창 억제와 UTF-8 강제가 여기 걸린다.
    fn base(&self) -> Command {
        let mut cmd = win::command("wsl.exe");
        // WSL_UTF8: 미설정 시 wsl.exe 자신의 메시지가 UTF-16LE 로 나온다 (§6.4).
        cmd.env("WSL_UTF8", "1");
        cmd.args(["-d", self.distro.as_str()]);
        cmd
    }

    /// 배포판 안에서 프로그램 하나를 직접 실행하고 결과를 모은다.
    ///
    /// `exec()` 로 부르는 것은 **coreutils 로 한정한다** (`printenv`, `cp` 등).
    /// 로그인 셸을 거치지 않으므로 `/opt/conda/bin` 이 PATH 에 없어 `chewBBACA.py`
    /// 나 `python3` 는 찾지 못한다.
    fn exec(&self, argv: &[&str]) -> Result<win::Captured> {
        let mut cmd = self.base();
        cmd.arg(EXEC_FLAG);
        cmd.args(argv);
        win::capture(&mut cmd)
    }

    /// 로그인 셸에서 스크립트를 실행한다.
    ///
    /// `-l` 이 필요한 이유: rootfs 의 micromamba 활성화가 프로필에서 일어난다(§8.2).
    /// 활성화가 안 되면 `chewBBACA.py` 를 찾지 못한다.
    fn bash(&self, script: &str) -> Result<win::Captured> {
        let mut cmd = self.base();
        cmd.args(login_shell_argv(script));
        win::capture(&mut cmd)
    }

    fn home(&self) -> Result<String> {
        {
            let cached = self.home.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(h) = cached.as_ref() {
                return Ok(h.clone());
            }
        }
        let out = self.exec(&["printenv", "HOME"])?.require_success()?;
        let home = out.stdout.trim().to_string();
        if home.is_empty() {
            return Err(Error::BackendUnavailable(
                "배포판에서 $HOME 을 확인하지 못했습니다".into(),
            ));
        }
        *self.home.lock().unwrap_or_else(|e| e.into_inner()) = Some(home.clone());
        Ok(home)
    }

    fn work_dir(&self, job_id: &str) -> Result<String> {
        Ok(format!("{}/work/{}", self.home()?, job_id))
    }

    fn schema_root(&self) -> Result<String> {
        Ok(format!("{}/schemas", self.home()?))
    }

    /// 입력을 ext4 로 복사한다 (§5.2 규칙 1).
    fn stage_input(&self, work: &str, src_backend: &str, sink: &EventSink) -> Result<()> {
        sink(RunEvent::Notice(
            "입력 파일을 WSL 내부(ext4)로 복사하는 중...".into(),
        ));
        let script = format!(
            "set -e
             rm -rf {work}/input
             mkdir -p {work}/input {work}/output
             cp -a {src}/. {work}/input/
             find {work}/input -maxdepth 1 -type f | wc -l",
            work = sh_quote(work),
            src = sh_quote(src_backend),
        );
        let out = self.bash(&script)?.require_success()?;
        let count = out.stdout.trim();
        sink(RunEvent::Notice(format!("입력 {count}개 파일 준비 완료")));
        Ok(())
    }

    /// 파일 하나만 ext4 로 복사한다 (ExtractCgMLST 처럼 입력이 파일인 모듈용).
    ///
    /// 폴더째 복사할 이유가 없다 — `results_alleles.tsv` 옆에는 수 MB 짜리
    /// `cds_coordinates.tsv` 가 같이 있고, 그건 이 모듈이 읽지 않는다.
    /// 반환값은 복사된 파일의 백엔드 경로다.
    fn stage_file(&self, work: &str, src_backend: &str, sink: &EventSink) -> Result<String> {
        sink(RunEvent::Notice(
            "입력 파일을 WSL 내부(ext4)로 복사하는 중...".into(),
        ));
        let script = format!(
            "set -e
             rm -rf {work}/input
             mkdir -p {work}/input {work}/output
             cp -a {src} {work}/input/
             basename {src}",
            work = sh_quote(work),
            src = sh_quote(src_backend),
        );
        let out = self.bash(&script)?.require_success()?;
        let name = out.stdout.trim().to_string();
        if name.is_empty() {
            return Err(Error::Other("입력 파일 이름을 확인하지 못했습니다".into()));
        }
        sink(RunEvent::Notice(format!("입력 파일 준비 완료: {name}")));
        Ok(format!("{work}/input/{name}"))
    }

    /// 파일 여러 개를 ext4 로 복사한다 (JoinProfiles 처럼 입력이 목록인 모듈용).
    ///
    /// 같은 이름의 파일이 섞이면 서로 덮어쓰므로 번호를 붙여 구분한다 —
    /// 여러 번 돌린 결과는 하나같이 `results_alleles.tsv` 다.
    fn stage_files(
        &self,
        work: &str,
        srcs: &[String],
        sink: &EventSink,
    ) -> Result<Vec<String>> {
        sink(RunEvent::Notice(format!(
            "입력 파일 {}개를 WSL 내부(ext4)로 복사하는 중...",
            srcs.len()
        )));

        let mut lines = String::from("set -e\n");
        lines.push_str(&format!(
            "rm -rf {w}/input\nmkdir -p {w}/input {w}/output\n",
            w = sh_quote(work)
        ));
        let mut staged = Vec::new();
        for (i, src) in srcs.iter().enumerate() {
            let dest = format!("{work}/input/{i}.tsv");
            lines.push_str(&format!(
                "cp -a {} {}\n",
                sh_quote(src),
                sh_quote(&dest)
            ));
            staged.push(dest);
        }
        self.bash(&lines)?.require_success()?;
        sink(RunEvent::Notice("입력 파일 준비 완료".into()));
        Ok(staged)
    }

    /// 스테이징이 미리 만들어 둔 빈 `output` 을 도로 지운다.
    ///
    /// SchemaEvaluator·AlleleCallEvaluator 는 `-o` 가 **이미 있으면 아무것도 하지 않고
    /// 거부한다** — `Output directory already exists.` 한 줄과 exit 1 이 전부다
    /// (chewBBACA.py 의 `run_evaluate_schema`/`run_evaluate_calls`, 2026-08-11 실측).
    /// 그래서 이 두 모듈에서는 chewBBACA 자신이 폴더를 만들게 둬야 한다.
    ///
    /// `rm -rf` 가 아니라 `rmdir` 인 것은 의도적이다. 비어 있을 때만 지워지므로,
    /// 어떤 경로 착오가 있어도 남의 결과를 날리지 않는다.
    fn drop_empty_output(&self, work: &str) -> Result<()> {
        self.bash(&format!(
            "rmdir {}/output 2>/dev/null || true",
            sh_quote(work)
        ))?;
        Ok(())
    }

    /// 실행 스크립트. 반환값의 종료 코드는 chewBBACA 자신의 것이다.
    ///
    /// `setsid --wait` 가 두 가지를 동시에 해준다 —
    /// 새 세션(=새 프로세스 그룹) 생성, 그리고 자식 종료 코드 전달.
    /// 내부 bash 의 `$$` 가 곧 PGID 이므로 그 값을 표식과 함께 먼저 출력한다.
    ///
    /// **chewBBACA 의 출력은 앱의 파이프가 아니라 파일로 간다.** 이게 §6.3("작업은
    /// 앱보다 오래 산다")의 실제 조건이다. 앱이 닫히면 stdout 파이프가 닫히고, 거기에
    /// 쓰던 프로세스는 SIGPIPE 로 죽는다 — `setsid` 로 프로세스 그룹을 분리해도
    /// 출력 대상이 앱의 파이프면 소용이 없다(2026-08-10 에 실측으로 확인했다).
    /// 파일에 쓰게 하고 `tail` 로 중계하면, 앱이 죽어도 죽는 것은 `tail` 뿐이다.
    fn spawn_and_stream(&self, work: &str, argv: &[String], sink: &EventSink) -> Result<i32> {
        let command_line = argv
            .iter()
            .map(|a| sh_quote(a))
            .collect::<Vec<_>>()
            .join(" ");

        // `tail --pid` 는 그 프로세스가 끝나면 **남은 내용을 흘린 뒤** 스스로 끝난다.
        // 직접 kill 하면 마지막 몇 줄(요약 표, "Finished at")을 놓친다.
        let script = format!(
            "set -o pipefail
             export PYTHONUNBUFFERED=1
             cd {work}
             export CHEWIE_CMD={cmd}
             LOG={work}/{log}
             : > \"$LOG\"
             setsid --wait bash -c 'echo \"{marker} $$\"; eval \"$CHEWIE_CMD\"' >> \"$LOG\" 2>&1 &
             CHILD=$!
             tail -n +1 -f --pid=\"$CHILD\" \"$LOG\"
             wait \"$CHILD\"",
            work = sh_quote(work),
            cmd = sh_quote(&command_line),
            marker = PGID_MARKER,
            log = RUN_LOG,
        );

        let mut cmd = self.base();
        cmd.args(login_shell_argv(script.as_str()))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");

        let out_sink = sink.clone();
        let out_thread = thread::spawn(move || pump(stdout, &out_sink, false));
        let err_sink = sink.clone();
        let err_thread = thread::spawn(move || pump(stderr, &err_sink, true));

        let status = child.wait()?;
        let _ = out_thread.join();
        let _ = err_thread.join();

        Ok(status.code().unwrap_or(-1))
    }

    /// 결과를 Windows 로 되돌린다 (§5.2 규칙 3).
    fn collect_output(&self, work: &str, host_dest: &Path, sink: &EventSink) -> Result<String> {
        std::fs::create_dir_all(host_dest)?;
        let dest_backend = self.to_backend_path(host_dest)?;
        sink(RunEvent::Notice(
            "결과를 Windows 폴더로 회수하는 중...".into(),
        ));
        let script = format!(
            "set -e
             mkdir -p {dest}
             cp -a {work}/output/. {dest}/",
            work = sh_quote(work),
            dest = sh_quote(&dest_backend),
        );
        self.bash(&script)?.require_success()?;
        Ok(host_dest.to_string_lossy().to_string())
    }

    /// CreateSchema 직후 스키마 디렉터리를 조사한다.
    fn inspect_schema(&self, schema_id: &str, name: &str, path: &str) -> Result<CreatedSchema> {
        // `.trn` 은 스키마 루트가 아니라 `schema_seed/` 안에 놓인다 (3.5.4 에서 확인).
        // 루트도 함께 보는 것은 다른 버전이나 PrepExternalSchema 산출물을 대비한 것이다.
        let script = format!(
            "set -e
             ls -1 {p}/*.trn {p}/schema_seed/*.trn 2>/dev/null | head -n 1 || true
             echo '---'
             ls -1 {p}/schema_seed/*.fasta 2>/dev/null | wc -l",
            p = sh_quote(path)
        );
        let out = self.bash(&script)?;
        let mut parts = out.stdout.split("---");
        let ptf = parts
            .next()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let loci_count = parts
            .next()
            .and_then(|s| s.trim().parse::<i64>().ok())
            .filter(|n| *n > 0);

        Ok(CreatedSchema {
            schema_id: schema_id.to_string(),
            name: name.to_string(),
            backend_path: path.to_string(),
            ptf,
            loci_count,
        })
    }
}

impl ChewieRunner for WslRunner {
    fn ensure_ready(&self) -> Result<()> {
        // §7.3 의 낙관적 시도와 같은 명령. 실패하면 온보딩 게이트로 내려간다.
        let out = self.exec(&["true"]).map_err(|e| {
            Error::BackendUnavailable(format!(
                "배포판 '{}' 를 실행할 수 없습니다: {e}",
                self.distro
            ))
        })?;
        if !out.ok() {
            return Err(Error::BackendUnavailable(format!(
                "배포판 '{}' 응답 없음: {}",
                self.distro,
                out.stderr.trim()
            )));
        }
        Ok(())
    }

    fn status(&self) -> BackendStatus {
        match self.ensure_ready() {
            Ok(()) => {
                let version = self
                    .bash("chewBBACA.py --version 2>&1 | head -n 1")
                    .ok()
                    .filter(|o| o.ok())
                    .map(|o| o.stdout.trim().to_string())
                    .filter(|s| !s.is_empty());
                BackendStatus {
                    ready: version.is_some(),
                    distro: self.distro.clone(),
                    cpu_count: self.cpu_count().ok(),
                    detail: match &version {
                        Some(v) => format!("chewBBACA 확인됨: {v}"),
                        None => "배포판은 응답하지만 chewBBACA 를 찾지 못했습니다".into(),
                    },
                    chewbbaca_version: version,
                }
            }
            Err(e) => BackendStatus {
                ready: false,
                distro: self.distro.clone(),
                chewbbaca_version: None,
                cpu_count: None,
                detail: e.to_string(),
            },
        }
    }

    fn to_backend_path(&self, host: &Path) -> Result<String> {
        validate_host_path(host)?;
        let raw = host.to_string_lossy().to_string();
        let out = self.exec(&["wslpath", "-a", raw.as_str()])?;
        if !out.ok() {
            return Err(Error::InvalidInput(format!(
                "경로를 변환할 수 없습니다: {raw} ({})",
                out.stderr.trim()
            )));
        }
        let converted = out.stdout.trim().to_string();
        if converted.is_empty() {
            return Err(Error::InvalidInput(format!(
                "경로 변환 결과가 비었습니다: {raw}"
            )));
        }
        Ok(converted)
    }

    fn cpu_count(&self) -> Result<u32> {
        let out = self.exec(&["nproc"])?.require_success()?;
        out.stdout
            .trim()
            .parse::<u32>()
            .map_err(|_| Error::Other(format!("nproc 파싱 실패: {}", out.stdout.trim())))
    }

    fn run(&self, job_id: &str, spec: &JobSpec, sink: &EventSink) -> Result<RunOutcome> {
        // CreateSchema 는 결과 폴더가 선택이라 비어 있을 수 있다.
        if !spec.output_dir.trim().is_empty() {
            validate_host_path(Path::new(&spec.output_dir))?;
        }

        let work = self.work_dir(job_id)?;

        // 입력 모양이 모듈마다 다르다 — 폴더 · 파일 하나 · 파일 여럿 · 스테이징 없음.
        // 파일 하나만 옮기면 될 것을 폴더째 복사하지 않도록 여기서 갈라 둔다.
        let mut staged_files: Vec<String> = Vec::new();
        let staged_input = match &spec.params {
            _ if spec.input_dir().is_some() => {
                let dir = spec.input_dir().expect("바로 위에서 확인했다");
                validate_host_path(Path::new(dir))?;
                let input_backend = self.to_backend_path(Path::new(dir))?;
                self.stage_input(&work, &input_backend, sink)?;
                format!("{work}/input")
            }
            ModuleParams::ExtractCgMLST { profiles_file, .. } => {
                validate_host_path(Path::new(profiles_file))?;
                let src = self.to_backend_path(Path::new(profiles_file))?;
                self.stage_file(&work, &src, sink)?
            }
            ModuleParams::RemoveGenes {
                profiles_file,
                genes_list,
                ..
            } => {
                // 표와 목록 두 개. 둘 다 작아 함께 옮긴다.
                for p in [profiles_file, genes_list] {
                    validate_host_path(Path::new(p))?;
                }
                let srcs = [
                    self.to_backend_path(Path::new(profiles_file))?,
                    self.to_backend_path(Path::new(genes_list))?,
                ];
                staged_files = self.stage_files(&work, &srcs, sink)?;
                staged_files[0].clone()
            }
            ModuleParams::JoinProfiles { profiles_files, .. } => {
                let mut srcs = Vec::new();
                for p in profiles_files {
                    validate_host_path(Path::new(p))?;
                    srcs.push(self.to_backend_path(Path::new(p))?);
                }
                staged_files = self.stage_files(&work, &srcs, sink)?;
                staged_files.first().cloned().unwrap_or_default()
            }
            ModuleParams::SchemaEvaluator { .. } => {
                // 입력이 앱 저장소의 스키마다. 이미 ext4 안에 있으므로 복사하지 않는다.
                // `output` 은 만들지 않는다 — 이 모듈은 그게 있으면 거부한다
                // (`drop_empty_output` 참조).
                self.bash(&format!("mkdir -p {}", sh_quote(&work)))?
                    .require_success()?;
                String::new()
            }
            _ => return Err(Error::Other("입력 모양을 알 수 없는 모듈입니다".into())),
        };

        let cpu = match spec.cpu {
            Some(n) if n > 0 => n,
            // WSL 내부 nproc 을 쓴다. Windows 논리 코어 수와 다를 수 있다 (§6.4).
            _ => self.cpu_count().unwrap_or(1),
        };

        let mut args = BackendArgs {
            // 폴더 모듈은 `{work}/input`, 파일 모듈은 복사된 파일의 경로가 들어온다.
            input: staged_input,
            output: format!("{work}/output"),
            cds_input: spec.cds_input(),
            cpu,
            ..Default::default()
        };

        // 모듈별로 산출물이 가는 곳이 다르다.
        //  - CreateSchema:  스키마는 앱이 소유하므로 ~/schemas 로 직접 만든다 (§4.4)
        //  - AlleleCall:    결과는 사용자 것이므로 work/output 을 거쳐 회수한다
        //  - ExtractCgMLST: 마찬가지로 회수한다 (cgMLSTschema*.txt 가 다음 실행의 입력이 된다)
        let mut created_schema_target: Option<(String, String)> = None;
        match &spec.params {
            ModuleParams::CreateSchema { ptf, .. } => {
                // 조정(reconciliation)이 같은 값을 다시 만들어야 하므로 규칙은 공용이다.
                let schema_id = schema_id_for(job_id, spec);
                let path = format!("{}/{}", self.schema_root()?, schema_id);
                args.output = path.clone();
                if let Some(p) = ptf {
                    args.ptf = Some(self.to_backend_path(Path::new(p))?);
                }
                created_schema_target = Some((schema_id, schema_name_of(spec).to_string()));
            }
            ModuleParams::AlleleCall {
                schema_id,
                loci_list,
                ..
            } => {
                args.schema = Some(format!("{}/{}", self.schema_root()?, schema_id));
                if let Some(gl) = loci_list {
                    args.loci_list = Some(self.to_backend_path(Path::new(gl))?);
                }
            }
            ModuleParams::ExtractCgMLST { thresholds, .. } => {
                // 스키마도 어셈블리도 필요 없다. 입력 TSV 는 위에서 이미 스테이징했다.
                args.thresholds = thresholds.clone();
            }
            ModuleParams::RemoveGenes { keep_instead, .. } => {
                // `-o` 가 폴더가 아니라 파일이다. 회수는 폴더째 하므로 그 안에 만든다.
                args.genes_list = staged_files.get(1).cloned();
                args.output = format!("{work}/output/results_alleles_filtered.tsv");
                args.flag = *keep_instead;
            }
            ModuleParams::JoinProfiles { common_only, .. } => {
                args.inputs = staged_files.clone();
                args.output = format!("{work}/output/joined_profiles.tsv");
                args.flag = *common_only;
            }
            ModuleParams::SchemaEvaluator {
                schema_id,
                loci_reports,
            } => {
                args.schema = Some(format!("{}/{}/schema_seed", self.schema_root()?, schema_id));
                args.flag = *loci_reports;
                self.drop_empty_output(&work)?;
            }
            ModuleParams::AlleleCallEvaluator { schema_id, .. } => {
                args.schema = Some(format!("{}/{}/schema_seed", self.schema_root()?, schema_id));
                self.drop_empty_output(&work)?;
            }
            ModuleParams::PrepExternalSchema { ptf, .. } => {
                // **`-o` 를 스키마 폴더가 아니라 그 안의 `schema_seed` 로 겨눈다.**
                // CreateSchema 는 `-o` 아래에 `schema_seed/` 를 만들지만 이 모듈은
                // 변환된 loci FASTA 를 `-o` 바로 아래에 푼다(3.5.4 소스로 확인).
                // 여기서 한 겹 내려주면 앱의 나머지(AlleleCall 의 -g, loci 계수,
                // 내보내기)가 두 모듈을 구분하지 않아도 된다.
                let schema_id = schema_id_for(job_id, spec);
                let path = format!("{}/{}", self.schema_root()?, schema_id);
                args.output = format!("{path}/schema_seed");
                if let Some(p) = ptf {
                    args.ptf = Some(self.to_backend_path(Path::new(p))?);
                }
                created_schema_target =
                    Some((schema_id, schema_name_of(spec).to_string()));
            }
        }

        let argv = build_argv(spec.module(), &args);
        sink(RunEvent::Notice(format!("실행: {}", argv.join(" "))));

        let exit_code = self.spawn_and_stream(&work, &argv, sink)?;

        let mut outcome = RunOutcome {
            exit_code,
            work_dir: work.clone(),
            collected_to: None,
            created_schema: None,
        };

        if exit_code != 0 {
            return Ok(outcome);
        }

        match created_schema_target {
            Some((schema_id, name)) => {
                let path = args.output.clone();
                outcome.created_schema = Some(self.inspect_schema(&schema_id, &name, &path)?);
                sink(RunEvent::Notice(format!(
                    "스키마 '{name}' 가 앱 저장소에 등록되었습니다. 내보내기는 스키마 화면에서 할 수 있습니다."
                )));
            }
            None => {
                outcome.collected_to =
                    Some(self.collect_output(&work, Path::new(&spec.output_dir), sink)?);
            }
        }

        Ok(outcome)
    }

    fn cancel(&self, handle: &JobHandle) -> Result<()> {
        let pgid = handle.pgid.ok_or_else(|| {
            Error::Other("아직 프로세스 그룹이 만들어지지 않아 취소할 수 없습니다".into())
        })?;

        // 실행 중인 wsl.exe 를 죽이는 것이 아니라 **별도 프로세스**로 그룹에
        // 시그널을 보낸다 (§6.2). TERM 으로 정리할 기회를 준 뒤 KILL 로 확실히 끝낸다.
        let script = format!(
            "kill -TERM -{pgid} 2>/dev/null || true
             for _ in $(seq 1 20); do
               kill -0 -{pgid} 2>/dev/null || exit 0
               sleep 0.25
             done
             kill -KILL -{pgid} 2>/dev/null || true"
        );
        self.bash(&script)?;
        Ok(())
    }

    fn is_alive(&self, handle: &JobHandle) -> Result<bool> {
        let Some(pgid) = handle.pgid else {
            return Ok(false);
        };
        let out = self.bash(&format!(
            "kill -0 -{pgid} 2>/dev/null && echo alive || echo dead"
        ))?;
        Ok(out.stdout.trim() == "alive")
    }

    fn output_produced(&self, job_id: &str, spec: &JobSpec) -> Result<bool> {
        // CreateSchema 의 산출물은 작업 디렉터리가 아니라 스키마 저장소에 있다.
        // 여기를 작업 디렉터리로 보면 성공한 고아 작업이 전부 실패로 확정된다.
        let target = if spec.module().produces_schema() {
            format!(
                "{}/{}/schema_seed",
                self.schema_root()?,
                schema_id_for(job_id, spec)
            )
        } else {
            format!("{}/output", self.work_dir(job_id)?)
        };

        let out = self.bash(&format!(
            "ls -A {} 2>/dev/null | head -n 1",
            sh_quote(&target)
        ))?;
        Ok(!out.stdout.trim().is_empty())
    }

    fn list_schema_dirs(&self) -> Result<Vec<String>> {
        let root = self.schema_root()?;
        let out = self.bash(&format!(
            "mkdir -p {r} && ls -1 {r} 2>/dev/null || true",
            r = sh_quote(&root)
        ))?;
        Ok(out
            .stdout
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    fn schema_path(&self, schema_id: &str) -> Result<String> {
        Ok(format!("{}/{}", self.schema_root()?, schema_id))
    }

    fn inspect_schema_dir(&self, schema_id: &str, name: &str) -> Result<CreatedSchema> {
        let path = format!("{}/{}", self.schema_root()?, schema_id);
        self.inspect_schema(schema_id, name, &path)
    }

    fn remove_schema(&self, schema_id: &str) -> Result<()> {
        // 상위 디렉터리 탈출을 막는다. schema_id 는 DB 에서 오지만 방어한다.
        if schema_id.contains('/') || schema_id.contains("..") || schema_id.is_empty() {
            return Err(Error::InvalidInput(format!(
                "잘못된 스키마 ID: {schema_id}"
            )));
        }
        let path = format!("{}/{}", self.schema_root()?, schema_id);
        self.bash(&format!("rm -rf {}", sh_quote(&path)))?
            .require_success()?;
        Ok(())
    }

    fn import_schema_dir(&self, host_src: &Path, schema_id: &str) -> Result<String> {
        validate_host_path(host_src)?;
        let src = self.to_backend_path(host_src)?;
        let dest = format!("{}/{}", self.schema_root()?, schema_id);

        // `set -e` 와 `[ -e ]` 로 덮어쓰기를 막는다. mkdir -p 로 만들고 복사하면
        // 이미 있는 스키마 위에 파일이 섞여 들어간다.
        let script = format!(
            "set -e
             if [ -e {dest} ]; then echo '이미 같은 이름의 스키마가 있습니다' >&2; exit 3; fi
             mkdir -p {dest}
             cp -a {src}/. {dest}/
             ls -1 {dest}/schema_seed/*.fasta 2>/dev/null | wc -l",
            src = sh_quote(&src),
            dest = sh_quote(&dest),
        );
        let out = self.bash(&script)?;
        if !out.ok() {
            // 실패하면 반쯤 복사된 디렉터리를 남기지 않는다.
            let _ = self.bash(&format!("rm -rf {}", sh_quote(&dest)));
            return Err(Error::Other(format!(
                "스키마를 들여오지 못했습니다.\n{}",
                out.stderr.trim()
            )));
        }
        Ok(dest)
    }

    fn export_dir(&self, backend_path: &str, host_dest: &Path) -> Result<()> {
        std::fs::create_dir_all(host_dest)?;
        let dest = self.to_backend_path(host_dest)?;
        self.bash(&format!(
            "set -e
             mkdir -p {dest}
             cp -a {src}/. {dest}/",
            src = sh_quote(backend_path),
            dest = sh_quote(&dest)
        ))?
        .require_success()?;
        Ok(())
    }

    fn cleanup_work(&self, job_id: &str) -> Result<()> {
        let work = self.work_dir(job_id)?;
        self.bash(&format!("rm -rf {}", sh_quote(&work)))?;
        Ok(())
    }

    fn create_training_file(&self, host_genome: &Path, host_output: &Path) -> Result<()> {
        validate_host_path(host_genome)?;
        validate_host_path(host_output)?;

        // 입력은 게놈 하나(수 MB), 출력은 `.trn` 하나(수십 KB)다. 스테이징하지 않고
        // `/mnt/c` 를 그대로 쓴다 — §5.2 의 복사 규칙은 수천 개 파일을 다루는
        // 모듈 이야기이고, 여기서는 복사가 오히려 왕복을 늘린다.
        let genome = self.to_backend_path(host_genome)?;

        // 출력 파일은 아직 없다. `wslpath` 에 없는 파일을 그대로 넘기지 않고
        // **부모 폴더를 변환한 뒤 이름을 붙인다** — 부모까지 없으면 변환이 실패한다.
        let parent = host_output.parent().ok_or_else(|| {
            Error::InvalidInput(format!(
                "training file 을 둘 폴더를 알 수 없습니다: {}",
                host_output.display()
            ))
        })?;
        let file_name = host_output
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| Error::InvalidInput("training file 이름이 비어 있습니다".into()))?;
        let output = format!("{}/{}", self.to_backend_path(parent)?, file_name);

        let script = training_argv(&genome, &output)
            .iter()
            .map(|a| sh_quote(a))
            .collect::<Vec<_>>()
            .join(" ");

        // pyrodigal 이 실패하면 Python 트레이스백이 그대로 올라온다. 사용자에게는
        // 그것만으로 아무 도움이 안 되므로 무엇을 확인해야 하는지를 앞에 붙인다.
        self.bash(&script)?.require_success().map_err(|e| {
            Error::Other(format!(
                "training file 을 만들지 못했습니다.\n고른 파일이 게놈 FASTA 가 맞는지, 서열이 충분히 긴지 확인하세요.\n\n{e}"
            ))
        })?;
        Ok(())
    }
}

/// 자식 프로세스 출력 스트림을 한 줄씩 sink 로 흘려보낸다.
///
/// `\n` 뿐 아니라 `\r` 도 구분자로 취급한다 — 진행률 표시줄이 캐리지 리턴만
/// 쓰는 경우 `\n` 만 기다리면 출력이 통째로 멈춰 보인다.
fn pump<R: Read>(reader: R, sink: &EventSink, is_stderr: bool) {
    let mut reader = std::io::BufReader::new(reader);
    let mut acc: Vec<u8> = Vec::with_capacity(256);
    let mut chunk = [0u8; 4096];

    let emit = |bytes: &[u8]| {
        let line = String::from_utf8_lossy(bytes).trim_end().to_string();
        if line.is_empty() {
            return;
        }
        if let Some(rest) = line.strip_prefix(PGID_MARKER) {
            if let Ok(pgid) = rest.trim().parse::<i32>() {
                sink(RunEvent::Pgid(pgid));
            }
            return; // 표식 줄은 로그에 남기지 않는다
        }
        sink(if is_stderr {
            RunEvent::Stderr(line)
        } else {
            RunEvent::Stdout(line)
        });
    };

    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                for &b in &chunk[..n] {
                    if b == b'\n' || b == b'\r' {
                        if !acc.is_empty() {
                            emit(&acc);
                            acc.clear();
                        }
                    } else {
                        acc.push(b);
                    }
                }
            }
            Err(_) => break,
        }
    }
    if !acc.is_empty() {
        emit(&acc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_shell_never_uses_the_double_dash_separator() {
        // `--` 로 되돌리면 배포판 기본 셸이 스크립트를 한 번 더 파싱해 인용부호를 먹는다.
        // 그러면 실패하지 않고 **조용히 아무것도 하지 않는다** — 회귀를 여기서 막는다.
        let argv = login_shell_argv("echo hi");
        assert_eq!(argv[0], "-e", "wsl.exe 는 -e 로 직접 exec 해야 한다");
        assert_ne!(argv[0], "--");
        assert_eq!(argv[1..], ["bash", "-lc", "echo hi"]);
    }

    #[test]
    fn login_shell_keeps_the_l_flag() {
        // `-l` 이 빠지면 micromamba 가 활성화되지 않아 chewBBACA.py 를 못 찾는다 (§8.2).
        assert!(login_shell_argv("x").contains(&"-lc"));
    }
}
