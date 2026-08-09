//! Schema Store (§4.4).
//!
//! **스키마는 WSL 내부(`~/schemas/`)에 상주하며 앱이 소유한다.**
//! AlleleCall 이 신규 allele 을 스키마 디렉터리에 계속 추가하기 때문에,
//! Windows 측에 두면 9p 오버헤드가 실행마다 누적된다. 그래서 GUI 는
//! 목록·삭제·내보내기 인터페이스만 제공한다.
//!
//! 각 스키마는 Prodigal training file(`.trn`)을 내부에 보관한다. AlleleCall 시
//! `--ptf` 를 다시 넘기지 않으며, **결과 일관성을 위해 동일 training file 을
//! 계속 사용해야 한다.**

use std::path::Path;
use std::sync::Arc;

use crate::db::Db;
use crate::error::{Error, Result};
use crate::models::SchemaInfo;
use crate::runner::ChewieRunner;
use crate::util::now_iso;

pub struct SchemaStore {
    db: Arc<Db>,
    runner: Arc<dyn ChewieRunner>,
}

impl SchemaStore {
    pub fn new(db: Arc<Db>, runner: Arc<dyn ChewieRunner>) -> Self {
        Self { db, runner }
    }

    /// DB 와 백엔드 디렉터리를 대조한 목록.
    ///
    /// 둘이 어긋나는 두 경우를 여기서 정리한다.
    /// * DB 에는 있는데 디렉터리가 없다 → 사용자가 WSL 에서 지웠거나 재설치했다. 행을 지운다.
    /// * 디렉터리는 있는데 DB 에 없다 → DB 유실. 이름만으로 복구해 목록에 넣는다.
    pub fn list(&self) -> Result<Vec<SchemaInfo>> {
        let dirs = self.runner.list_schema_dirs().unwrap_or_default();
        let recorded = self.db.list_schemas()?;

        let mut result = Vec::new();
        for info in recorded {
            if dirs.contains(&info.schema_id) {
                result.push(info);
            } else {
                // 백엔드에 실체가 없는 항목은 목록에 남겨봐야 실행 시 실패할 뿐이다.
                let _ = self.db.delete_schema(&info.schema_id);
            }
        }

        for dir in dirs {
            if result.iter().any(|s| s.schema_id == dir) {
                continue;
            }
            let recovered = SchemaInfo {
                schema_id: dir.clone(),
                name: dir.clone(),
                created_at: now_iso(),
                created_by_job: None,
                backend_path: String::new(),
                ptf: None,
                loci_count: None,
            };
            let _ = self.db.insert_schema(&recovered);
            result.push(recovered);
        }

        Ok(result)
    }

    pub fn get(&self, schema_id: &str) -> Result<SchemaInfo> {
        self.db
            .get_schema(schema_id)?
            .ok_or_else(|| Error::InvalidInput(format!("스키마를 찾을 수 없습니다: {schema_id}")))
    }

    /// 스키마를 지운다. 되돌릴 수 없으므로 UI 에서 확인을 받은 뒤 호출한다.
    pub fn delete(&self, schema_id: &str) -> Result<()> {
        self.runner.remove_schema(schema_id)?;
        self.db.delete_schema(schema_id)?;
        Ok(())
    }

    /// 스키마 전체를 Windows 폴더로 내보낸다 (백업·다른 PC 이관용).
    pub fn export(&self, schema_id: &str, dest: &Path) -> Result<()> {
        let info = self.get(schema_id)?;
        let backend_path = if info.backend_path.is_empty() {
            // 복구된 항목은 경로가 비어 있다. 규칙대로 재구성한다.
            self.runner.schema_path(schema_id)?
        } else {
            info.backend_path
        };
        self.runner.export_dir(&backend_path, dest)
    }
}
