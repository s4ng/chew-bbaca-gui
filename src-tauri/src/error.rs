//! 앱 전역 에러 타입.
//!
//! Tauri command 는 `Result<T, E>` 의 `E: Serialize` 를 요구하므로
//! 프런트에는 `{ kind, message }` 형태로 직렬화된다. `kind` 는 UI 가 분기에
//! 사용하고(예: 권한 상승 거부 → 수동 안내 폴백), `message` 는 사용자에게 보여준다.

use serde::{Serialize, Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("데이터베이스 오류: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("{0}")]
    Json(#[from] serde_json::Error),

    /// WSL 배포판이 없거나 기동하지 않는다. 온보딩으로 되돌려야 한다.
    #[error("WSL 배포판을 사용할 수 없습니다: {0}")]
    BackendUnavailable(String),

    /// `wsl.exe` 하위 명령이 0 이 아닌 코드로 끝났다.
    #[error("WSL 명령 실패 (exit {code}): {stderr}")]
    WslCommand { code: i32, stderr: String },

    /// UAC 대화상자에서 사용자가 [아니오] 를 눌렀다. 수동 안내로 폴백한다.
    #[error("관리자 권한 요청이 거부되었습니다")]
    ElevationDenied,

    /// 입력 단계에서 걸러야 하는 값 (UNC 경로 등).
    #[error("{0}")]
    InvalidInput(String),

    #[error("작업을 찾을 수 없습니다: {0}")]
    JobNotFound(String),

    #[error("다운로드 실패: {0}")]
    Download(String),

    /// rootfs 체크섬 불일치 — 받은 파일을 폐기해야 한다.
    #[error("체크섬이 일치하지 않습니다 (기대 {expected}, 실제 {actual})")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("{0}")]
    Other(String),
}

impl Error {
    /// 프런트가 분기에 사용하는 안정적인 식별자.
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Io(_) => "io",
            Error::Db(_) => "db",
            Error::Json(_) => "json",
            Error::BackendUnavailable(_) => "backend-unavailable",
            Error::WslCommand { .. } => "wsl-command",
            Error::ElevationDenied => "elevation-denied",
            Error::InvalidInput(_) => "invalid-input",
            Error::JobNotFound(_) => "job-not-found",
            Error::Download(_) => "download",
            Error::ChecksumMismatch { .. } => "checksum-mismatch",
            Error::Other(_) => "other",
        }
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Other(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Other(s.to_string())
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Error", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}
