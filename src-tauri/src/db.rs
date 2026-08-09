//! SQLite 영속 계층 (§3.3, §6.1).
//!
//! 작업 상태가 프로세스 메모리가 아니라 여기에 있는 이유는 단순하다 —
//! **작업은 앱보다 오래 산다.** 앱을 닫아도 WSL 안의 chewBBACA 는 계속 돌고,
//! 다시 켰을 때 그 사실을 알아내는 유일한 근거가 이 파일이다.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::Result;
use crate::models::{Job, JobStatus, Module, SchemaInfo};
use crate::util::now_iso;

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        // WAL: 로그 기록 중에도 UI 쿼리가 막히지 않게 한다.
        // `journal_mode` 는 값을 행으로 돌려주므로 execute 계열이 아니라
        // execute_batch 로 던져야 한다 (execute 는 "결과가 반환됨" 으로 실패한다).
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    /// 테스트용 인메모리 DB.
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let db = Self {
            conn: Mutex::new(Connection::open_in_memory()?),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.lock();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS jobs (
                job_id      TEXT PRIMARY KEY,
                module      TEXT NOT NULL,
                status      TEXT NOT NULL,
                args        TEXT NOT NULL,
                created_at  TEXT NOT NULL,
                started_at  TEXT,
                finished_at TEXT,
                pgid        INTEGER,
                work_dir    TEXT,
                log_path    TEXT,
                output_path TEXT,
                exit_code   INTEGER,
                error       TEXT,
                progress    REAL
            );
            CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
            CREATE INDEX IF NOT EXISTS idx_jobs_created ON jobs(created_at DESC);

            CREATE TABLE IF NOT EXISTS schemas (
                schema_id      TEXT PRIMARY KEY,
                name           TEXT NOT NULL,
                created_at     TEXT NOT NULL,
                created_by_job TEXT,
                backend_path   TEXT NOT NULL,
                ptf            TEXT,
                loci_count     INTEGER
            );

            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        // Mutex 가 poison 되는 경우는 다른 스레드 패닉뿐이다. 그때도 DB 자체는
        // 멀쩡하므로 계속 진행한다 — 여기서 앱을 죽이면 복구 경로가 사라진다.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ------------------------------------------------------------ jobs

    pub fn insert_job(&self, job: &Job) -> Result<()> {
        self.lock().execute(
            "INSERT INTO jobs (job_id, module, status, args, created_at, log_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                job.job_id,
                job.module.cli_name(),
                job.status.as_str(),
                job.args,
                job.created_at,
                job.log_path,
            ],
        )?;
        Ok(())
    }

    pub fn get_job(&self, job_id: &str) -> Result<Option<Job>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM jobs WHERE job_id = ?1"))?;
        let job = stmt.query_row([job_id], row_to_job).optional()?;
        Ok(job)
    }

    pub fn list_jobs(&self, limit: i64) -> Result<Vec<Job>> {
        let conn = self.lock();
        let mut stmt =
            conn.prepare(&format!("SELECT {COLUMNS} FROM jobs ORDER BY created_at DESC LIMIT ?1"))?;
        let rows = stmt.query_map([limit], row_to_job)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn list_by_status(&self, status: JobStatus) -> Result<Vec<Job>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM jobs WHERE status = ?1 ORDER BY created_at ASC"
        ))?;
        let rows = stmt.query_map([status.as_str()], row_to_job)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// 큐에서 다음 작업 하나를 꺼낸다. 동시 실행은 1건으로 직렬화한다 (§4.2).
    pub fn next_queued(&self) -> Result<Option<Job>> {
        Ok(self.list_by_status(JobStatus::Queued)?.into_iter().next())
    }

    pub fn mark_running(&self, job_id: &str, work_dir: &str) -> Result<()> {
        self.lock().execute(
            "UPDATE jobs SET status = 'running', started_at = ?2, work_dir = ?3 WHERE job_id = ?1",
            params![job_id, now_iso(), work_dir],
        )?;
        Ok(())
    }

    /// PGID 는 획득 즉시 기록한다. 이 값이 없으면 취소도 고아 판정도 불가능하다.
    pub fn set_pgid(&self, job_id: &str, pgid: i32) -> Result<()> {
        self.lock().execute(
            "UPDATE jobs SET pgid = ?2 WHERE job_id = ?1",
            params![job_id, pgid],
        )?;
        Ok(())
    }

    pub fn set_work_dir(&self, job_id: &str, work_dir: &str) -> Result<()> {
        self.lock().execute(
            "UPDATE jobs SET work_dir = ?2 WHERE job_id = ?1",
            params![job_id, work_dir],
        )?;
        Ok(())
    }

    pub fn set_progress(&self, job_id: &str, fraction: f32) -> Result<()> {
        self.lock().execute(
            "UPDATE jobs SET progress = ?2 WHERE job_id = ?1",
            params![job_id, fraction],
        )?;
        Ok(())
    }

    pub fn finish_job(
        &self,
        job_id: &str,
        status: JobStatus,
        exit_code: Option<i32>,
        error: Option<&str>,
        output_path: Option<&str>,
    ) -> Result<()> {
        self.lock().execute(
            "UPDATE jobs
                SET status = ?2, finished_at = ?3, exit_code = ?4, error = ?5,
                    output_path = COALESCE(?6, output_path)
              WHERE job_id = ?1",
            params![job_id, status.as_str(), now_iso(), exit_code, error, output_path],
        )?;
        Ok(())
    }

    pub fn delete_job(&self, job_id: &str) -> Result<()> {
        self.lock()
            .execute("DELETE FROM jobs WHERE job_id = ?1", [job_id])?;
        Ok(())
    }

    // ------------------------------------------------------------ schemas

    pub fn insert_schema(&self, s: &SchemaInfo) -> Result<()> {
        self.lock().execute(
            "INSERT OR REPLACE INTO schemas
               (schema_id, name, created_at, created_by_job, backend_path, ptf, loci_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                s.schema_id,
                s.name,
                s.created_at,
                s.created_by_job,
                s.backend_path,
                s.ptf,
                s.loci_count
            ],
        )?;
        Ok(())
    }

    pub fn list_schemas(&self) -> Result<Vec<SchemaInfo>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT schema_id, name, created_at, created_by_job, backend_path, ptf, loci_count
               FROM schemas ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SchemaInfo {
                schema_id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
                created_by_job: r.get(3)?,
                backend_path: r.get(4)?,
                ptf: r.get(5)?,
                loci_count: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_schema(&self, schema_id: &str) -> Result<Option<SchemaInfo>> {
        Ok(self
            .list_schemas()?
            .into_iter()
            .find(|s| s.schema_id == schema_id))
    }

    pub fn delete_schema(&self, schema_id: &str) -> Result<()> {
        self.lock()
            .execute("DELETE FROM schemas WHERE schema_id = ?1", [schema_id])?;
        Ok(())
    }

    // ------------------------------------------------------------ settings

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.lock();
        let v = conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get::<_, String>(0)
            })
            .optional()?;
        Ok(v)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.lock().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

const COLUMNS: &str = "job_id, module, status, args, created_at, started_at, finished_at, \
                       pgid, work_dir, log_path, output_path, exit_code, error, progress";

fn row_to_job(r: &Row<'_>) -> rusqlite::Result<Job> {
    let module: String = r.get(1)?;
    let status: String = r.get(2)?;
    Ok(Job {
        job_id: r.get(0)?,
        module: Module::parse(&module).unwrap_or(Module::CreateSchema),
        status: JobStatus::parse(&status),
        args: r.get(3)?,
        created_at: r.get(4)?,
        started_at: r.get(5)?,
        finished_at: r.get(6)?,
        pgid: r.get(7)?,
        work_dir: r.get(8)?,
        log_path: r.get(9)?,
        output_path: r.get(10)?,
        exit_code: r.get(11)?,
        error: r.get(12)?,
        progress: r.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_job(id: &str) -> Job {
        Job {
            job_id: id.into(),
            module: Module::AlleleCall,
            status: JobStatus::Queued,
            args: "{}".into(),
            created_at: now_iso(),
            started_at: None,
            finished_at: None,
            pgid: None,
            work_dir: None,
            log_path: Some(format!("C:/logs/{id}.log")),
            output_path: None,
            exit_code: None,
            error: None,
            progress: None,
        }
    }

    #[test]
    fn job_roundtrip_and_lifecycle() {
        let db = Db::open_memory().unwrap();
        db.insert_job(&sample_job("j1")).unwrap();

        assert_eq!(db.next_queued().unwrap().unwrap().job_id, "j1");

        db.mark_running("j1", "/home/chewie/work/j1").unwrap();
        db.set_pgid("j1", 4242).unwrap();
        let job = db.get_job("j1").unwrap().unwrap();
        assert_eq!(job.status, JobStatus::Running);
        assert_eq!(job.pgid, Some(4242));

        db.finish_job("j1", JobStatus::Completed, Some(0), None, Some("C:/out"))
            .unwrap();
        let job = db.get_job("j1").unwrap().unwrap();
        assert!(job.status.is_terminal());
        assert_eq!(job.output_path.as_deref(), Some("C:/out"));
        assert!(db.next_queued().unwrap().is_none());
    }

    #[test]
    fn settings_upsert() {
        let db = Db::open_memory().unwrap();
        assert!(db.get_setting("distro").unwrap().is_none());
        db.set_setting("distro", "chewie-env").unwrap();
        db.set_setting("distro", "chewie-env-2").unwrap();
        assert_eq!(db.get_setting("distro").unwrap().unwrap(), "chewie-env-2");
    }
}
