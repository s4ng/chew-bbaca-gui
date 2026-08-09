//! 앱 설정. SQLite `settings` 테이블에 JSON 한 덩어리로 보관한다.
//!
//! 키를 잘게 쪼개지 않는 이유: 설정 항목이 늘어날 때마다 마이그레이션을
//! 쓰고 싶지 않기 때문이다. 필드가 없으면 `serde` 기본값이 채운다.

use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::env::RootfsSource;
use crate::error::Result;

const KEY: &str = "settings";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// 전용 배포판 이름. 사용자 기존 배포판과 충돌하지 않는 고정값이다.
    pub distro: String,
    pub rootfs: RootfsSource,
    /// 완료된 작업의 `~/work/{job_id}` 를 남겨둘지 여부. 기본은 정리.
    pub keep_work_dir: bool,
    /// 미지정이면 WSL 내부 `nproc` 을 쓴다 (§6.4).
    pub default_cpu: Option<u32>,
    /// 새 작업 폼의 기본 출력 폴더
    pub last_output_dir: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            distro: "chewie-env".into(),
            rootfs: RootfsSource {
                // TODO(release): 첫 릴리스 태그가 정해지면 실제 URL·체크섬으로 교체한다.
                // 값이 비어 있으면 온보딩 ③ 단계에서 다운로드를 시작하지 않고
                // "배포 준비 중" 안내를 띄운다.
                url: String::new(),
                sha256: String::new(),
                file_name: "chewie-rootfs-3.5.4.tar.gz".into(),
                version: "3.5.4".into(),
            },
            keep_work_dir: false,
            default_cpu: None,
            last_output_dir: None,
        }
    }
}

impl Settings {
    pub fn load(db: &Db) -> Settings {
        match db.get_setting(KEY) {
            Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_default(),
            _ => Settings::default(),
        }
    }

    pub fn save(&self, db: &Db) -> Result<()> {
        db.set_setting(KEY, &serde_json::to_string(self)?)?;
        Ok(())
    }

    /// rootfs 배포 정보가 채워져 있는지. 비어 있으면 자동 설치를 시도하지 않는다.
    pub fn rootfs_ready(&self) -> bool {
        !self.rootfs.url.is_empty() && self.rootfs.sha256.len() == 64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_not_installable_until_release_metadata_is_filled() {
        assert!(!Settings::default().rootfs_ready());
    }

    #[test]
    fn roundtrips_through_db() {
        let db = Db::open_memory().unwrap();
        let mut s = Settings::default();
        s.keep_work_dir = true;
        s.default_cpu = Some(4);
        s.save(&db).unwrap();

        let loaded = Settings::load(&db);
        assert!(loaded.keep_work_dir);
        assert_eq!(loaded.default_cpu, Some(4));
        assert_eq!(loaded.distro, "chewie-env");
    }

    #[test]
    fn unknown_or_missing_fields_fall_back_to_defaults() {
        let db = Db::open_memory().unwrap();
        db.set_setting(KEY, r#"{"keepWorkDir":true}"#).unwrap();
        let loaded = Settings::load(&db);
        assert!(loaded.keep_work_dir);
        assert_eq!(loaded.distro, "chewie-env");
    }
}
