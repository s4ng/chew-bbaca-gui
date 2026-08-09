//! Windows 측 디렉터리 레이아웃 (ARCHITECTURE.md §5.3).
//!
//! ```text
//! %LOCALAPPDATA%\ChewieApp\
//! ├── wsl\ext4.vhdx      # chewie-env 배포판 실체
//! ├── app.db             # SQLite
//! ├── logs\{job_id}.log  # 작업 로그 (DB 에는 경로만)
//! └── cache\rootfs-*.tar.gz
//! ```
//!
//! `%LOCALAPPDATA%` 를 쓰는 이유: `ProgramData`·`Program Files` 는 관리자 권한을
//! 요구해 perUser 설치 경험을 깨뜨린다.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub db: PathBuf,
    pub logs: PathBuf,
    pub cache: PathBuf,
    /// `wsl --import` 대상 디렉터리 (ext4.vhdx 가 여기 생성된다)
    pub wsl: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> Result<Self> {
        let base = local_app_data()?;
        let root = base.join("ChewieApp");
        Ok(Self {
            db: root.join("app.db"),
            logs: root.join("logs"),
            cache: root.join("cache"),
            wsl: root.join("wsl"),
            root,
        })
    }

    /// 앱 시작 시 한 번 호출한다. 이미 존재하면 아무 일도 하지 않는다.
    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [&self.root, &self.logs, &self.cache, &self.wsl] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn log_path(&self, job_id: &str) -> PathBuf {
        self.logs.join(format!("{job_id}.log"))
    }

    pub fn rootfs_cache(&self, file_name: &str) -> PathBuf {
        self.cache.join(file_name)
    }
}

fn local_app_data() -> Result<PathBuf> {
    if let Ok(v) = std::env::var("LOCALAPPDATA") {
        if !v.is_empty() {
            return Ok(PathBuf::from(v));
        }
    }
    // WSL 이나 CI 에서 테스트로 돌릴 때의 폴백.
    if let Ok(home) = std::env::var("USERPROFILE") {
        return Ok(PathBuf::from(home).join("AppData").join("Local"));
    }
    Err(Error::Other(
        "%LOCALAPPDATA% 를 확인할 수 없습니다".into(),
    ))
}

/// 입력 단계 게이트 (§5.4).
///
/// UNC 경로와 매핑 네트워크 드라이브는 **미지원으로 명시**한다. `wslpath` 가
/// 변환하지 못할 뿐 아니라, 변환되더라도 9p 위에서 수천 개 파일을 다루게 되어
/// 실행 시간이 예측 불가능해진다.
pub fn validate_host_path(path: &Path) -> Result<()> {
    let s = path.to_string_lossy();

    if s.starts_with("\\\\") || s.starts_with("//") {
        return Err(Error::InvalidInput(format!(
            "네트워크(UNC) 경로는 지원하지 않습니다. 로컬 드라이브로 복사한 뒤 다시 시도하세요: {s}"
        )));
    }
    if !path.is_absolute() {
        return Err(Error::InvalidInput(format!("절대 경로가 필요합니다: {s}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unc_paths() {
        assert!(validate_host_path(Path::new(r"\\server\share\data")).is_err());
    }

    #[test]
    fn accepts_korean_and_spaced_paths() {
        // §5.4 에서 초기부터 테스트하기로 한 케이스.
        assert!(validate_host_path(Path::new(r"C:\Users\홍 길동\어셈블리")).is_ok());
    }

    #[test]
    fn rejects_relative_paths() {
        assert!(validate_host_path(Path::new("data")).is_err());
    }
}
