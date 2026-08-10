//! Job Manager — 작업 수명주기를 소유하는 단일 지점 (§4.2, §6).
//!
//! 동시 실행은 **기본 1건으로 직렬화**한다. chewBBACA 가 `--cpu` 로 이미 전
//! 코어를 점유하므로 병렬 실행은 이득이 없고, 오히려 서로의 I/O 를 방해한다.
//!
//! 상태가 이 구조체의 필드가 아니라 SQLite 에 있다는 점이 중요하다. 앱이
//! 종료되어도 WSL 안의 작업은 계속 돈다 — 다시 켰을 때 그 사실을 알아낼
//! 유일한 근거가 DB 에 남은 `status=running` 과 `pgid` 다 (§6.3).

use std::collections::HashSet;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::db::Db;
use crate::error::{Error, Result};
use crate::models::{
    Job, JobSpec, JobStatus, LogEvent, LogStream, Module, ProgressEvent, SchemaInfo, StateEvent,
    EVENT_LOG, EVENT_PROGRESS, EVENT_STATE,
};
use crate::paths::{validate_host_path, AppPaths};
use crate::runner::progress::ProgressParser;
use crate::runner::{ChewieRunner, EventSink, JobHandle, RunEvent};
use crate::settings::Settings;
use crate::util::now_iso;

/// 고아 작업을 지켜볼 때의 폴링 간격. 40분짜리 작업 기준이므로 촘촘할 필요가 없다.
const ADOPT_POLL: Duration = Duration::from_secs(5);

pub struct JobManager {
    app: AppHandle,
    db: Arc<Db>,
    runner: Arc<dyn ChewieRunner>,
    paths: AppPaths,
    /// 실행 중인 작업의 핸들. 취소는 이 값을 통해서만 가능하다.
    current: Mutex<Option<JobHandle>>,
    /// 실행 슬롯 1개. `true` 면 워커 스레드가 살아 있다.
    busy: AtomicBool,
    /// 사용자가 취소를 요청한 작업. 종료 코드가 0 이 아니어도 `failed` 가 아니라
    /// `cancelled` 로 확정하기 위한 표시다.
    cancelling: Mutex<HashSet<String>>,
    /// 이번 실행에서 이어받은(= 이전 실행이 남긴) 작업들.
    ///
    /// 조정은 프로세스당 한 번만 돌지만 **배너는 작업이 끝날 때까지 떠 있어야 한다.**
    /// 조정의 반환값에만 의존하면 화면을 한 번 옮겼다 돌아오는 순간 배너가 사라진다.
    adopted: Mutex<HashSet<String>>,
    /// 조정을 이미 돌렸는지. **프로세스당 한 번만** 돈다 (§6.3).
    ///
    /// 프런트가 [작업] 화면을 열 때마다 `jobs_reconcile` 을 부르는데, 이 빗장이 없으면
    /// 화면을 오갈 때마다 조정이 다시 돌아 **지금 이 프로세스가 실행 중인 작업**을
    /// 고아로 오판한다. 조정의 대상은 언제나 *이전* 실행이 남긴 작업뿐이다.
    reconciled: AtomicBool,
}

impl JobManager {
    pub fn new(
        app: AppHandle,
        db: Arc<Db>,
        runner: Arc<dyn ChewieRunner>,
        paths: AppPaths,
    ) -> Arc<Self> {
        Arc::new(Self {
            app,
            db,
            runner,
            paths,
            current: Mutex::new(None),
            busy: AtomicBool::new(false),
            cancelling: Mutex::new(HashSet::new()),
            adopted: Mutex::new(HashSet::new()),
            reconciled: AtomicBool::new(false),
        })
    }

    // ------------------------------------------------------------ 큐

    /// 작업을 큐에 넣는다. 슬롯이 비어 있으면 즉시 시작된다.
    pub fn submit(self: &Arc<Self>, spec: JobSpec) -> Result<String> {
        // 입력 게이트는 여기서 한 번만 통과시킨다. UNC 경로 등은 Runner 까지
        // 내려가기 전에 걸러야 사용자가 원인을 이해할 수 있다 (§5.4).
        if spec.module.requires_output_dir() || !spec.output_dir.trim().is_empty() {
            validate_host_path(std::path::Path::new(&spec.output_dir))?;
        }
        if spec.module.takes_input_dir() {
            validate_host_path(std::path::Path::new(&spec.input_dir))?;
        }
        if spec.module == Module::AlleleCall {
            if spec.schema_id.is_none() {
                return Err(Error::InvalidInput("스키마를 선택하세요".into()));
            }
            if let Some(gl) = spec.loci_list.as_deref().filter(|s| !s.trim().is_empty()) {
                validate_host_path(std::path::Path::new(gl))?;
                let info = crate::commands::inspect_loci_list(gl.to_string())?;
                if !info.looks_valid {
                    return Err(Error::InvalidInput(format!(
                        "loci 목록 파일이 아닙니다{}.\nExtractCgMLST 가 만든 cgMLSTschema*.txt 처럼 한 줄에 loci 이름 하나만 있는 파일을 선택하세요.",
                        if info.tabbed { " (탭으로 나뉜 표입니다)" } else { " (비어 있습니다)" }
                    )));
                }
            }
        }
        if spec.module == Module::ExtractCgMLST {
            let file = spec.profiles_file.as_deref().unwrap_or("");
            if file.trim().is_empty() {
                return Err(Error::InvalidInput(
                    "AlleleCall 결과 파일(results_alleles.tsv)을 선택하세요".into(),
                ));
            }
            validate_host_path(std::path::Path::new(file))?;
            // 폼에서 이미 걸렀더라도 여기서 한 번 더 본다. 잘못된 표를 넣으면
            // chewBBACA 가 거절하지 않고 각 행을 균주로 취급해 오래 헛돈다.
            let info = crate::commands::inspect_profiles_file(file.to_string())?;
            if !info.looks_valid {
                return Err(Error::InvalidInput(format!(
                    "이 파일은 AlleleCall 의 allelic profile 표가 아닙니다 (첫 열이 '{}', 열 {}개).\nAlleleCall 결과 폴더의 results_alleles.tsv 를 선택하세요.",
                    info.first_column,
                    info.loci + 1
                )));
            }
        }

        let job_id = Uuid::new_v4().to_string();
        let log_path = self.paths.log_path(&job_id);
        let job = Job {
            job_id: job_id.clone(),
            module: spec.module,
            status: JobStatus::Queued,
            args: serde_json::to_string(&spec)?,
            created_at: now_iso(),
            started_at: None,
            finished_at: None,
            pgid: None,
            work_dir: None,
            log_path: Some(log_path.to_string_lossy().to_string()),
            output_path: None,
            exit_code: None,
            error: None,
            progress: None,
        };
        self.db.insert_job(&job)?;
        self.emit_state(&job_id, JobStatus::Queued, None);
        self.maybe_start();
        Ok(job_id)
    }

    /// 슬롯이 비어 있으면 다음 큐 항목을 워커 스레드에서 실행한다.
    fn maybe_start(self: &Arc<Self>) {
        if self.busy.swap(true, Ordering::SeqCst) {
            return; // 이미 누군가 돌고 있다
        }
        let next = match self.db.next_queued() {
            Ok(Some(job)) => job,
            _ => {
                self.busy.store(false, Ordering::SeqCst);
                return;
            }
        };

        let me = Arc::clone(self);
        thread::spawn(move || {
            me.run_job(next);
            me.busy.store(false, Ordering::SeqCst);
            // 큐에 남은 것이 있으면 이어서 실행한다.
            me.maybe_start();
        });
    }

    // ------------------------------------------------------------ 실행

    fn run_job(self: &Arc<Self>, job: Job) {
        let job_id = job.job_id.clone();

        let spec: JobSpec = match serde_json::from_str(&job.args) {
            Ok(s) => s,
            Err(e) => {
                self.finalize(&job_id, JobStatus::Failed, None, Some(&format!("작업 인자를 해석할 수 없습니다: {e}")), None);
                return;
            }
        };

        let _ = self.db.mark_running(&job_id, "");
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = Some(JobHandle {
            job_id: job_id.clone(),
            work_dir: String::new(),
            pgid: None,
        });
        self.emit_state(&job_id, JobStatus::Running, None);

        // 진행률 단계표가 모듈마다 다르므로 여기서 모듈을 넘긴다 (progress.rs).
        let sink = self.make_sink(&job_id, spec.module);
        let result = self.runner.run(&job_id, &spec, &sink);

        let cancelled = self
            .cancelling
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&job_id);

        match result {
            Ok(outcome) => {
                let _ = self.db.set_work_dir(&job_id, &outcome.work_dir);

                if cancelled {
                    self.finalize(&job_id, JobStatus::Cancelled, Some(outcome.exit_code), None, None);
                } else if outcome.exit_code == 0 {
                    if let Some(created) = outcome.created_schema {
                        let info = SchemaInfo {
                            schema_id: created.schema_id,
                            name: created.name,
                            created_at: now_iso(),
                            created_by_job: Some(job_id.clone()),
                            backend_path: created.backend_path,
                            ptf: created.ptf,
                            loci_count: created.loci_count,
                        };
                        if let Err(e) = self.db.insert_schema(&info) {
                            self.log(&job_id, LogStream::App, &format!("스키마 등록 실패: {e}"));
                        }
                    }
                    // 회수한 결과가 없는 모듈(CreateSchema)도 결과 폴더를 지정했다면
                    // 최소한 로그 사본은 남긴다 — 폴더를 받아놓고 비워두지 않는다.
                    let landed = outcome
                        .collected_to
                        .clone()
                        .or_else(|| self.copy_log_to_output(&job_id, &spec.output_dir));

                    self.finalize(&job_id, JobStatus::Completed, Some(0), None, landed.as_deref());
                    self.cleanup_if_configured(&job_id);
                } else {
                    self.finalize(
                        &job_id,
                        JobStatus::Failed,
                        Some(outcome.exit_code),
                        Some(&format!(
                            "chewBBACA 가 종료 코드 {} 로 끝났습니다. 로그를 확인하세요.",
                            outcome.exit_code
                        )),
                        None,
                    );
                }
            }
            Err(e) => {
                let status = if cancelled {
                    JobStatus::Cancelled
                } else {
                    JobStatus::Failed
                };
                self.log(&job_id, LogStream::App, &e.to_string());
                self.finalize(&job_id, status, None, Some(&e.to_string()), None);
            }
        }

        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// 로그 라인을 파일에 쓰고 동시에 UI 이벤트로 방출하는 싱크 (§4.2).
    fn make_sink(self: &Arc<Self>, job_id: &str, module: Module) -> EventSink {
        let me = Arc::clone(self);
        let jid = job_id.to_string();
        let parser = Mutex::new(ProgressParser::for_module(module));

        Arc::new(move |event: RunEvent| match event {
            RunEvent::Pgid(pgid) => {
                // 획득 즉시 기록한다. 이 값이 없으면 취소도 고아 판정도 못 한다.
                let _ = me.db.set_pgid(&jid, pgid);
                if let Some(handle) = me
                    .current
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .as_mut()
                {
                    handle.pgid = Some(pgid);
                }
            }
            RunEvent::Stdout(line) => {
                me.log(&jid, LogStream::Stdout, &line);
                me.observe_progress(&parser, &jid, &line);
            }
            RunEvent::Stderr(line) => {
                me.log(&jid, LogStream::Stderr, &line);
                me.observe_progress(&parser, &jid, &line);
            }
            RunEvent::Notice(line) => me.log(&jid, LogStream::App, &line),
        })
    }

    fn observe_progress(&self, parser: &Mutex<ProgressParser>, job_id: &str, line: &str) {
        let parsed = parser
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .observe(line);
        if let Some((fraction, label)) = parsed {
            let _ = self.db.set_progress(job_id, fraction);
            let _ = self.app.emit(
                EVENT_PROGRESS,
                ProgressEvent {
                    job_id: job_id.to_string(),
                    fraction,
                    label,
                },
            );
        }
    }

    /// 로그는 파일이 진실이고 이벤트는 사본이다. UI 가 놓친 줄은 파일에서 읽는다.
    fn log(&self, job_id: &str, stream: LogStream, line: &str) {
        let path = self.paths.log_path(job_id);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(file, "{line}");
        }
        let _ = self.app.emit(
            EVENT_LOG,
            LogEvent {
                job_id: job_id.to_string(),
                stream,
                line: line.to_string(),
            },
        );
    }

    fn finalize(
        &self,
        job_id: &str,
        status: JobStatus,
        exit_code: Option<i32>,
        error: Option<&str>,
        output_path: Option<&str>,
    ) {
        let _ = self
            .db
            .finish_job(job_id, status, exit_code, error, output_path);
        self.emit_state(job_id, status, error.map(|s| s.to_string()));
    }

    /// 디스크의 스키마 디렉터리를 조사해 DB 에 등록한다.
    ///
    /// 정상 실행 경로는 `RunOutcome::created_schema` 를 그대로 쓰지만, 이어받은 작업은
    /// 그 값이 없다. 스키마 ID 는 작업 ID 와 이름만으로 결정되므로 다시 만들 수 있다.
    fn register_schema_from_disk(&self, job_id: &str, spec: &JobSpec) {
        let schema_id = crate::runner::schema_id_for(job_id, spec);
        let name = spec
            .schema_name
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| schema_id.clone());

        match self.runner.inspect_schema_dir(&schema_id, &name) {
            Ok(created) => {
                let info = SchemaInfo {
                    schema_id: created.schema_id,
                    name: created.name,
                    created_at: now_iso(),
                    created_by_job: Some(job_id.to_string()),
                    backend_path: created.backend_path,
                    ptf: created.ptf,
                    loci_count: created.loci_count,
                };
                if let Err(e) = self.db.insert_schema(&info) {
                    self.log(job_id, LogStream::App, &format!("스키마 등록 실패: {e}"));
                } else {
                    self.log(
                        job_id,
                        LogStream::App,
                        &format!("스키마 '{}' 를 등록했습니다.", info.name),
                    );
                }
            }
            Err(e) => self.log(job_id, LogStream::App, &format!("스키마 조사 실패: {e}")),
        }
    }

    /// 결과 폴더에 실행 로그 사본을 남기고 그 폴더 경로를 돌려준다.
    ///
    /// 회수할 산출물이 없는 모듈(CreateSchema)을 위한 것이다. 폴더를 필수로 받아놓고
    /// 아무것도 넣지 않으면 사용자는 빈 폴더를 보고 실패했다고 생각한다.
    /// 실패해도 작업 자체는 성공이므로 조용히 넘어간다.
    fn copy_log_to_output(&self, job_id: &str, output_dir: &str) -> Option<String> {
        let dir = output_dir.trim();
        if dir.is_empty() {
            return None;
        }
        let dest_dir = std::path::Path::new(dir);
        if let Err(e) = std::fs::create_dir_all(dest_dir) {
            self.log(job_id, LogStream::App, &format!("결과 폴더를 만들지 못했습니다: {e}"));
            return None;
        }
        let src = self.paths.log_path(job_id);
        let dest = dest_dir.join(format!("chewie_{job_id}.log"));
        match std::fs::copy(&src, &dest) {
            Ok(_) => {
                self.log(
                    job_id,
                    LogStream::App,
                    &format!("실행 로그를 결과 폴더에 남겼습니다: {}", dest.display()),
                );
                Some(dir.to_string())
            }
            Err(e) => {
                self.log(job_id, LogStream::App, &format!("로그 사본을 만들지 못했습니다: {e}"));
                None
            }
        }
    }

    fn cleanup_if_configured(&self, job_id: &str) {
        if Settings::load(&self.db).keep_work_dir {
            return;
        }
        if let Err(e) = self.runner.cleanup_work(job_id) {
            self.log(job_id, LogStream::App, &format!("임시 공간 정리 실패: {e}"));
        }
    }

    fn emit_state(&self, job_id: &str, status: JobStatus, message: Option<String>) {
        let _ = self.app.emit(
            EVENT_STATE,
            StateEvent {
                job_id: job_id.to_string(),
                status,
                message,
            },
        );
    }

    // ------------------------------------------------------------ 취소

    /// 취소 요청을 프로세스 그룹 종료로 변환한다 (§6.2).
    pub fn cancel(self: &Arc<Self>, job_id: &str) -> Result<()> {
        let job = self
            .db
            .get_job(job_id)?
            .ok_or_else(|| Error::JobNotFound(job_id.to_string()))?;

        match job.status {
            JobStatus::Queued => {
                // 아직 시작하지 않았으므로 죽일 프로세스가 없다. `error` 는 비워 둔다 —
                // 사용자가 스스로 취소한 것을 오류로 보여주면 안 된다.
                self.finalize(job_id, JobStatus::Cancelled, None, None, None);
                Ok(())
            }
            JobStatus::Running => {
                self.cancelling
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(job_id.to_string());
                self.log(job_id, LogStream::App, "취소 요청 — 프로세스 그룹을 종료합니다");

                let handle = JobHandle {
                    job_id: job_id.to_string(),
                    work_dir: job.work_dir.clone().unwrap_or_default(),
                    pgid: job.pgid,
                };
                self.runner.cancel(&handle)
            }
            _ => Ok(()), // 이미 종료된 작업
        }
    }

    // ------------------------------------------------------------ 조정

    /// 앱 시작 시 조정 (§6.3).
    ///
    /// `status=running` 인 레코드마다 PGID 생존을 확인해 셋 중 하나로 수렴시킨다.
    /// 살아 있는 작업은 사용자에게 선택(복구/종료)을 제시하기 위해 반환한다.
    pub fn reconcile(self: &Arc<Self>) -> Result<Vec<Job>> {
        // 두 번째 호출부터는 아무것도 하지 않는다. 프런트는 [작업] 화면을 열 때마다
        // 이 명령을 부르고, 그 사이 우리가 시작한 작업은 고아가 아니다.
        if self.reconciled.swap(true, Ordering::SeqCst) {
            return Ok(Vec::new());
        }

        // 이 프로세스가 방금 시작한 작업은 조정 대상이 아니다. `mark_running` 직후에는
        // PGID 가 아직 없어(표식이 도착하기 전) 살아 있는 작업이 죽은 것으로 보인다.
        let own = self
            .current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|h| h.job_id.clone());

        let mut alive = Vec::new();

        for job in self.db.list_by_status(JobStatus::Running)? {
            if own.as_deref() == Some(job.job_id.as_str()) {
                continue;
            }
            let handle = JobHandle {
                job_id: job.job_id.clone(),
                work_dir: job.work_dir.clone().unwrap_or_default(),
                pgid: job.pgid,
            };

            // 산출물이 어디 있는지는 모듈마다 다르므로 저장해 둔 인자를 다시 읽는다.
            // 인자를 해석하지 못하면 산출물 위치를 알 수 없어 실패로 확정할 수밖에 없다.
            let spec: Option<JobSpec> = serde_json::from_str(&job.args).ok();

            let is_alive = self.runner.is_alive(&handle).unwrap_or(false);
            if is_alive {
                // 생존 — stdout 은 이미 끊겼으므로 재연결할 수 없다.
                // 종료를 감시하다가 결과로 확정하는 것까지가 우리가 할 수 있는 일이다.
                *self.current.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
                self.busy.store(true, Ordering::SeqCst);
                self.adopted
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(job.job_id.clone());
                self.watch_adopted(job.job_id.clone(), spec);
                alive.push(job);
                continue;
            }

            // 사망 — 산출물 유무로 성공/실패를 가른다.
            let produced = spec
                .and_then(|s| self.runner.output_produced(&job.job_id, &s).ok())
                .unwrap_or(false);
            if produced {
                self.finalize(&job.job_id, JobStatus::Completed, None, None, None);
            } else {
                self.finalize(
                    &job.job_id,
                    JobStatus::Failed,
                    None,
                    Some("앱이 종료된 사이 프로세스가 사라졌고 결과도 없습니다"),
                    None,
                );
            }
        }

        self.maybe_start();
        Ok(alive)
    }

    /// 이어받은 작업 중 **아직 실행 중인 것**. 배너는 이 값으로 그린다.
    ///
    /// 조정과 분리해 둔 이유: 조정은 한 번만 돌지만 화면은 몇 번이고 다시 열린다.
    /// 작업이 끝나면 상태가 `running` 이 아니게 되어 자연히 목록에서 빠진다.
    pub fn adopted_jobs(&self) -> Result<Vec<Job>> {
        let ids: Vec<String> = self
            .adopted
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect();

        let mut out = Vec::new();
        for id in ids {
            if let Ok(Some(job)) = self.db.get_job(&id) {
                if job.status == JobStatus::Running {
                    out.push(job);
                }
            }
        }
        Ok(out)
    }

    /// 복구를 선택한 작업을 폴링으로 지켜본다.
    ///
    /// `spec` 은 종료 후 산출물을 어디서 찾을지 정하는 데 쓴다. 없으면(인자 해석 실패)
    /// 성공 여부를 판정할 근거가 없으므로 실패로 확정한다.
    fn watch_adopted(self: &Arc<Self>, job_id: String, spec: Option<JobSpec>) {
        let me = Arc::clone(self);
        thread::spawn(move || {
            me.log(
                &job_id,
                LogStream::App,
                "이전 실행에 다시 연결했습니다. 출력 스트림은 복구할 수 없어 완료 여부만 감시합니다.",
            );
            loop {
                thread::sleep(ADOPT_POLL);
                let handle = {
                    let guard = me.current.lock().unwrap_or_else(|e| e.into_inner());
                    match guard.as_ref() {
                        Some(h) if h.job_id == job_id => h.clone(),
                        _ => return, // 다른 작업이 슬롯을 차지했다 = 이미 끝난 것
                    }
                };
                if me.runner.is_alive(&handle).unwrap_or(false) {
                    continue;
                }
                let populated = spec
                    .as_ref()
                    .and_then(|s| me.runner.output_produced(&job_id, s).ok())
                    .unwrap_or(false);
                let status = if populated {
                    JobStatus::Completed
                } else {
                    JobStatus::Failed
                };

                // 이어받은 작업은 `RunOutcome` 이 없다. CreateSchema 였다면 여기서
                // 직접 등록하지 않으면 스키마가 DB 에 남지 않고, 나중에 목록이
                // 디렉터리만 보고 복구하면서 **이름과 loci 수를 잃는다.**
                if populated {
                    if let Some(s) = spec.as_ref() {
                        if s.module == Module::CreateSchema {
                            me.register_schema_from_disk(&job_id, s);
                        }
                    }
                }

                me.finalize(
                    &job_id,
                    status,
                    None,
                    if populated {
                        None
                    } else {
                        Some("프로세스가 결과 없이 종료되었습니다")
                    },
                    None,
                );
                *me.current.lock().unwrap_or_else(|e| e.into_inner()) = None;
                me.busy.store(false, Ordering::SeqCst);
                me.maybe_start();
                return;
            }
        });
    }

    // ------------------------------------------------------------ 조회

    pub fn list(&self, limit: i64) -> Result<Vec<Job>> {
        self.db.list_jobs(limit)
    }

    pub fn get(&self, job_id: &str) -> Result<Option<Job>> {
        self.db.get_job(job_id)
    }

    /// 로그 파일 전체를 읽는다. UI 가 이벤트를 놓쳤거나 앱을 다시 켠 경우에 쓴다.
    pub fn read_log(&self, job_id: &str) -> Result<String> {
        let path = self.paths.log_path(job_id);
        if !path.exists() {
            return Ok(String::new());
        }
        Ok(std::fs::read_to_string(path)?)
    }

    pub fn db(&self) -> &Arc<Db> {
        &self.db
    }

    pub fn runner(&self) -> &Arc<dyn ChewieRunner> {
        &self.runner
    }
}
