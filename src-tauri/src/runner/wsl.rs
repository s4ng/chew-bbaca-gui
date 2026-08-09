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
use crate::models::{JobSpec, Module};
use crate::paths::validate_host_path;
use crate::runner::cli::{build_argv, BackendArgs};
use crate::runner::{
    BackendStatus, ChewieRunner, CreatedSchema, EventSink, JobHandle, RunEvent, RunOutcome,
};
use crate::util::{sh_quote, slugify};
use crate::win;

/// PGID 를 stdout 으로 실어 보내기 위한 표식. 이 줄은 로그에 노출하지 않는다.
const PGID_MARKER: &str = "__CHEWIE_PGID__";

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
    fn exec(&self, argv: &[&str]) -> Result<win::Captured> {
        let mut cmd = self.base();
        cmd.arg("-e");
        cmd.args(argv);
        win::capture(&mut cmd)
    }

    /// 로그인 셸에서 스크립트를 실행한다.
    ///
    /// `-l` 이 필요한 이유: rootfs 의 micromamba 활성화가 프로필에서 일어난다(§8.2).
    /// 활성화가 안 되면 `chewBBACA.py` 를 찾지 못한다.
    fn bash(&self, script: &str) -> Result<win::Captured> {
        let mut cmd = self.base();
        cmd.args(["--", "bash", "-lc", script]);
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

    /// 실행 스크립트. 반환값의 종료 코드는 chewBBACA 자신의 것이다.
    ///
    /// `setsid --wait` 가 두 가지를 동시에 해준다 —
    /// 새 세션(=새 프로세스 그룹) 생성, 그리고 자식 종료 코드 전달.
    /// 내부 bash 의 `$$` 가 곧 PGID 이므로 그 값을 표식과 함께 먼저 출력한다.
    fn spawn_and_stream(&self, work: &str, argv: &[String], sink: &EventSink) -> Result<i32> {
        let command_line = argv
            .iter()
            .map(|a| sh_quote(a))
            .collect::<Vec<_>>()
            .join(" ");

        let script = format!(
            "set -o pipefail
             export PYTHONUNBUFFERED=1
             cd {work}
             export CHEWIE_CMD={cmd}
             setsid --wait bash -c 'echo \"{marker} $$\"; eval \"$CHEWIE_CMD\"'",
            work = sh_quote(work),
            cmd = sh_quote(&command_line),
            marker = PGID_MARKER,
        );

        let mut cmd = self.base();
        cmd.args(["--", "bash", "-lc", script.as_str()])
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
        sink(RunEvent::Notice("결과를 Windows 폴더로 회수하는 중...".into()));
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
        let script = format!(
            "set -e
             ls -1 {p}/*.trn 2>/dev/null | head -n 1 || true
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
            Error::BackendUnavailable(format!("배포판 '{}' 를 실행할 수 없습니다: {e}", self.distro))
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
            return Err(Error::InvalidInput(format!("경로 변환 결과가 비었습니다: {raw}")));
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
        validate_host_path(Path::new(&spec.input_dir))?;
        validate_host_path(Path::new(&spec.output_dir))?;

        let work = self.work_dir(job_id)?;
        let input_backend = self.to_backend_path(Path::new(&spec.input_dir))?;
        self.stage_input(&work, &input_backend, sink)?;

        let cpu = match spec.cpu {
            Some(n) if n > 0 => n,
            // WSL 내부 nproc 을 쓴다. Windows 논리 코어 수와 다를 수 있다 (§6.4).
            _ => self.cpu_count().unwrap_or(1),
        };

        let mut args = BackendArgs {
            input: format!("{work}/input"),
            output: format!("{work}/output"),
            cds_input: spec.cds_input,
            cpu,
            ..Default::default()
        };

        // 모듈별로 산출물이 가는 곳이 다르다.
        //  - CreateSchema: 스키마는 앱이 소유하므로 ~/schemas 로 직접 만든다 (§4.4)
        //  - AlleleCall:   결과는 사용자 것이므로 work/output 을 거쳐 회수한다
        let mut created_schema_target: Option<(String, String)> = None;
        match spec.module {
            Module::CreateSchema => {
                let name = spec
                    .schema_name
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| format!("schema-{job_id}"));
                let schema_id = format!("{}-{}", slugify(&name), &job_id[..8.min(job_id.len())]);
                let path = format!("{}/{}", self.schema_root()?, schema_id);
                args.output = path.clone();
                if let Some(ptf) = &spec.ptf {
                    args.ptf = Some(self.to_backend_path(Path::new(ptf))?);
                }
                created_schema_target = Some((schema_id, name));
            }
            Module::AlleleCall => {
                let schema_id = spec.schema_id.clone().ok_or_else(|| {
                    Error::InvalidInput("AlleleCall 에는 스키마 선택이 필요합니다".into())
                })?;
                args.schema = Some(format!("{}/{}", self.schema_root()?, schema_id));
                if let Some(gl) = &spec.loci_list {
                    args.loci_list = Some(self.to_backend_path(Path::new(gl))?);
                }
            }
        }

        let argv = build_argv(spec.module, &args);
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
        let out = self.bash(&format!("kill -0 -{pgid} 2>/dev/null && echo alive || echo dead"))?;
        Ok(out.stdout.trim() == "alive")
    }

    fn output_populated(&self, job_id: &str) -> Result<bool> {
        let work = self.work_dir(job_id)?;
        let out = self.bash(&format!(
            "ls -A {}/output 2>/dev/null | head -n 1",
            sh_quote(&work)
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

    fn remove_schema(&self, schema_id: &str) -> Result<()> {
        // 상위 디렉터리 탈출을 막는다. schema_id 는 DB 에서 오지만 방어한다.
        if schema_id.contains('/') || schema_id.contains("..") || schema_id.is_empty() {
            return Err(Error::InvalidInput(format!("잘못된 스키마 ID: {schema_id}")));
        }
        let path = format!("{}/{}", self.schema_root()?, schema_id);
        self.bash(&format!("rm -rf {}", sh_quote(&path)))?
            .require_success()?;
        Ok(())
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
