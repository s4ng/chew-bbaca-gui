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
    pub mcp: McpSettings,
}

/// 로컬 MCP 서버 설정 (`doc/MCP.md`).
///
/// 이 값들은 **앱이 소유한 것만** 다룬다 — 클라이언트 쪽 설정 파일은 건드리지 않고,
/// 사용자가 붙여넣을 조각만 만들어 준다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpSettings {
    pub enabled: bool,
    pub port: u16,
    /// 베어러 토큰. **비어 있으면 첫 기동에서 발급해 저장한다.**
    /// 기본값에 박아두면 모든 설치본이 같은 토큰을 쓰게 된다.
    pub token: String,
    /// 끄면 읽기 전용이 된다 — 실행 도구를 목록에서 빼고 호출도 거절한다.
    ///
    /// 승인 UI 를 만들지 않은 것은 의도된 선택이다. 앱을 보고 있지 않으면 작업이
    /// 그대로 멈추기 때문에, 자동 승인을 기본으로 두고 통째로 끄는 스위치만 준다.
    pub allow_run: bool,
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 8787,
            token: String::new(),
            allow_run: true,
        }
    }
}

impl McpSettings {
    /// 새 베어러 토큰. UUID v4 둘을 이어 붙여 64자리 16진수를 만든다.
    pub fn new_token() -> String {
        format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        )
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            distro: "chewie-env".into(),
            rootfs: RootfsSource {
                // 비어 있는 것이 정상이다. rootfs 는 인스톨러에 동봉되므로 받을 곳이 없다
                // (`tauri.bundle.json` → `provision.rs::bundled_rootfs`).
                // 직접 빌드한 이미지를 시험할 때만 로컬 경로나 http 주소를 채운다.
                url: String::new(),
                // 2026-08-09 `rootfs/build.sh 3.5.4` 산출물의 체크섬.
                // rootfs 를 다시 빌드하면 (`docker build` 재현성이 없으므로) 반드시 바뀐다.
                sha256: "9d1cb6e03626a646b5555895d10289b7fb6948eebaeb0d18b601b9de8dad10d8".into(),
                file_name: "chewie-rootfs-3.5.4.tar.gz".into(),
                version: "3.5.4".into(),
            },
            keep_work_dir: false,
            default_cpu: None,
            last_output_dir: None,
            mcp: McpSettings::default(),
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

    /// 체크섬이 형식상 쓸 수 있는 값인지. **어디서 가져오는지는 여기서 모른다** —
    /// 동봉본 존재 여부는 `Provisioner::rootfs_origin()` 이 판단한다.
    pub fn checksum_looks_valid(&self) -> bool {
        self.rootfs.sha256.len() == 64 && self.rootfs.sha256.chars().all(|c| c.is_ascii_hexdigit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_url_is_empty_so_the_bundled_image_is_used() {
        // URL 이 채워져 있으면 동봉본을 무시하게 된다 — 기본값은 비어 있어야 한다.
        assert!(Settings::default().rootfs.url.is_empty());
    }

    #[test]
    fn default_checksum_is_a_full_sha256() {
        assert!(Settings::default().checksum_looks_valid());
    }

    #[test]
    fn a_truncated_checksum_is_rejected() {
        let mut s = Settings::default();
        s.rootfs.sha256 = "9d1cb6e0".into();
        assert!(!s.checksum_looks_valid());
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

    #[test]
    fn settings_saved_before_mcp_existed_still_load() {
        // 0.2.x 에서 만들어진 행에는 mcp 키가 없다. 컨테이너 default 가 채워야 한다 —
        // 안 채워지면 앱이 설정을 통째로 기본값으로 되돌린다.
        let db = Db::open_memory().unwrap();
        db.set_setting(KEY, r#"{"distro":"chewie-env","keepWorkDir":true}"#)
            .unwrap();
        let loaded = Settings::load(&db);
        assert!(loaded.keep_work_dir, "기존 값이 살아 있어야 한다");
        assert!(loaded.mcp.enabled);
        assert_eq!(loaded.mcp.port, 8787);
    }

    #[test]
    fn a_fresh_token_is_not_baked_into_the_default() {
        // 기본값에 토큰을 박아두면 모든 설치본이 같은 토큰을 쓴다.
        assert!(McpSettings::default().token.is_empty());
        let a = McpSettings::new_token();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, McpSettings::new_token());
    }
}
