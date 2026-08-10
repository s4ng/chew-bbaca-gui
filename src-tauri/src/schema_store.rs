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
        for mut info in recorded {
            if !dirs.contains(&info.schema_id) {
                // 백엔드에 실체가 없는 항목은 목록에 남겨봐야 실행 시 실패할 뿐이다.
                let _ = self.db.delete_schema(&info.schema_id);
                continue;
            }
            // loci 수가 비어 있으면 지금 채운다. 예전에 디렉터리만 보고 복구된 항목이나,
            // 조사 시점에 아직 파일이 다 쓰이지 않았던 항목이 여기 해당한다.
            if info.loci_count.is_none() {
                if let Ok(found) = self
                    .runner
                    .inspect_schema_dir(&info.schema_id, &info.name)
                {
                    if found.loci_count.is_some() {
                        info.loci_count = found.loci_count;
                        info.ptf = found.ptf.or(info.ptf);
                        if info.backend_path.is_empty() {
                            info.backend_path = found.backend_path;
                        }
                        let _ = self.db.insert_schema(&info);
                    }
                }
            }
            result.push(info);
        }

        for dir in dirs {
            if result.iter().any(|s| s.schema_id == dir) {
                continue;
            }
            // 디렉터리만 남은 스키마. 표시 이름은 되살릴 수 없지만 loci 수와
            // training file 은 디렉터리를 보면 알 수 있다 — 빈 채로 두지 않는다.
            let probed = self.runner.inspect_schema_dir(&dir, &dir).ok();
            let recovered = SchemaInfo {
                schema_id: dir.clone(),
                name: dir.clone(),
                created_at: now_iso(),
                created_by_job: None,
                backend_path: probed
                    .as_ref()
                    .map(|p| p.backend_path.clone())
                    .unwrap_or_default(),
                ptf: probed.as_ref().and_then(|p| p.ptf.clone()),
                loci_count: probed.as_ref().and_then(|p| p.loci_count),
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

    /// 내보낸 스키마 폴더를 다시 들여온다.
    ///
    /// **`PrepExternalSchema` 와는 다른 물건이다.** 이것은 이 앱이 내보낸 것을
    /// 되돌리는 기능이고, 외부 형식의 스키마를 chewBBACA 형식으로 변환하는 것은
    /// 별개의 모듈이다. 그래서 여기서는 변환 없이 복사만 한다.
    ///
    /// 검증은 Windows 쪽에서 먼저 한다 — 잘못된 폴더를 고른 경우 WSL 을 거치지 않고
    /// 즉시 알려줄 수 있다.
    pub fn import(&self, src: &Path, name: &str) -> Result<SchemaInfo> {
        let display_name = name.trim();
        if display_name.is_empty() {
            return Err(Error::InvalidInput("스키마 이름을 입력하세요".into()));
        }
        if !src.is_dir() {
            return Err(Error::InvalidInput(format!(
                "폴더를 찾을 수 없습니다: {}",
                src.display()
            )));
        }

        // chewBBACA 스키마는 loci FASTA 가 `schema_seed/` 안에 있다.
        let seed = src.join("schema_seed");
        if !seed.is_dir() {
            return Err(Error::InvalidInput(format!(
                "chewBBACA 스키마 폴더가 아닙니다 — 안에 schema_seed 폴더가 없습니다.\n[스키마] → [내보내기] 로 만든 폴더를 그대로 고르세요.\n고른 폴더: {}",
                src.display()
            )));
        }
        let loci = std::fs::read_dir(&seed)?
            .flatten()
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("fasta"))
            })
            .count();
        if loci == 0 {
            return Err(Error::InvalidInput(
                "schema_seed 폴더에 loci FASTA 파일이 없습니다. 내보내기가 덜 끝난 폴더일 수 있습니다.".into(),
            ));
        }

        // 작업이 만든 것과 같은 규칙으로 ID 를 만든다. 뒤의 8자리는 충돌 회피용이다.
        let suffix = uuid::Uuid::new_v4().to_string();
        let schema_id = format!("{}-{}", crate::util::slugify(display_name), &suffix[..8]);
        let backend_path = self.runner.import_schema_dir(src, &schema_id)?;

        // 복사가 끝난 뒤 실제 값으로 등록한다. 원본을 세는 것보다 확실하다.
        let probed = self.runner.inspect_schema_dir(&schema_id, display_name).ok();
        let info = SchemaInfo {
            schema_id,
            name: display_name.to_string(),
            created_at: now_iso(),
            created_by_job: None,
            backend_path,
            ptf: probed.as_ref().and_then(|p| p.ptf.clone()),
            loci_count: probed.as_ref().and_then(|p| p.loci_count),
        };
        self.db.insert_schema(&info)?;
        Ok(info)
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
