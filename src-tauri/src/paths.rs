//! Windows 측 디렉터리 레이아웃 (ARCHITECTURE.md §5.3).
//!
//! ```text
//! %LOCALAPPDATA%\ChewieApp\
//! ├── wsl\ext4.vhdx      # chewie-env 배포판 실체
//! ├── app.db             # SQLite
//! ├── logs\{job_id}.log  # 작업 로그 (DB 에는 경로만)
//! ├── training\*.trn     # Prodigal training file 저장소
//! ├── cache\rootfs-*.tar.gz
//! └── location.txt       # 여기가 아닌 다른 곳을 쓸 때만 존재한다 (아래 참조)
//! ```
//!
//! `%LOCALAPPDATA%` 를 쓰는 이유: `ProgramData`·`Program Files` 는 관리자 권한을
//! 요구해 perUser 설치 경험을 깨뜨린다.
//!
//! ## 다른 드라이브로 옮기기
//!
//! 용량의 실체는 `wsl\ext4.vhdx` 하나이고 수 GB 로 자란다. C 드라이브가 작은
//! 기기에서는 이것을 D 드라이브에 두고 싶은 것이 당연하므로, 데이터 폴더 위치를
//! 사용자가 고를 수 있게 한다.
//!
//! **이 설정만은 `settings` 테이블에 둘 수 없다.** `app.db` 의 위치 자체가 이
//! 값으로 정해지므로 DB 를 열어야 알 수 있는 곳에 두면 닭-달걀이 된다. 그래서
//! 기본 위치에 포인터 파일 한 줄(`location.txt`)만 남기고, 그 파일이 실제 루트를
//! 가리킨다. 파일이 없으면 지금까지와 완전히 같이 동작한다.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// 데이터 폴더의 마지막 경로 요소. 사용자가 `D:\연구` 를 고르면
/// `D:\연구\ChewieApp` 이 된다.
///
/// **이름을 고정하는 것은 안전장치다.** 언인스톨 훅(`nsis/hooks.nsh`)이 이 폴더를
/// `RMDir /r` 로 통째로 지우므로, 지우기 직전에 경로가 이 이름으로 끝나는지
/// 한 번 더 확인한다. 이름을 바꾸려면 훅도 같이 고쳐야 한다.
pub const ROOT_DIR_NAME: &str = "ChewieApp";

/// 실제 데이터 폴더를 가리키는 포인터. **항상 기본 위치에 있다** — 이 파일을
/// 찾으려고 이 파일이 필요해지면 안 되기 때문이다.
const POINTER_FILE: &str = "location.txt";

/// 개발·테스트용 탈출구. 포인터 파일보다 우선한다.
const ENV_OVERRIDE: &str = "CHEWIE_APP_DIR";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub db: PathBuf,
    pub logs: PathBuf,
    pub cache: PathBuf,
    /// `wsl --import` 대상 디렉터리 (ext4.vhdx 가 여기 생성된다)
    pub wsl: PathBuf,
    /// Prodigal training file 저장소. 스키마와 달리 **Windows 쪽**에 둔다 —
    /// 파일 하나뿐이라 9p 비용이 없고, 사용자가 백업하거나 옮길 수 있어야 한다.
    pub training: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> Result<Self> {
        Ok(Self::at(resolve_root()?))
    }

    /// 루트를 직접 지정한다. 위치를 바꾸기 전에 대상 폴더를 검사할 때 쓴다.
    pub fn at(root: PathBuf) -> Self {
        Self {
            db: root.join("app.db"),
            logs: root.join("logs"),
            cache: root.join("cache"),
            wsl: root.join("wsl"),
            training: root.join("training"),
            root,
        }
    }

    /// 앱 시작 시 한 번 호출한다. 이미 존재하면 아무 일도 하지 않는다.
    ///
    /// 실패 메시지에 되돌리는 방법까지 적는 이유: 데이터 폴더를 외장 드라이브로
    /// 옮긴 뒤 그 드라이브를 뽑으면 여기서 앱이 뜨지 못한다. 그때 사용자가 할 수
    /// 있는 일이 화면에 없으면 앱을 다시 설치하는 수밖에 없다.
    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [
            &self.root,
            &self.logs,
            &self.cache,
            &self.wsl,
            &self.training,
        ] {
            std::fs::create_dir_all(dir).map_err(|e| {
                Error::Other(format!(
                    "데이터 폴더를 만들 수 없습니다: {}\n{e}\n\
                     외장·네트워크 드라이브라면 연결되어 있는지 확인하세요. \
                     기본 위치({})로 되돌리려면 아래 파일을 지우고 앱을 다시 실행하세요:\n{}",
                    dir.display(),
                    default_root()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| "%LOCALAPPDATA%\\ChewieApp".into()),
                    pointer_file()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| "%LOCALAPPDATA%\\ChewieApp\\location.txt".into()),
                ))
            })?;
        }
        Ok(())
    }

    /// 위치를 옮긴 첫 실행에서 기본 위치의 `app.db` 를 새 폴더로 가져온다.
    ///
    /// **DB 를 열기 전에만 부를 수 있다.** 열려 있는 SQLite 파일을 복사하면
    /// WAL 과 어긋난다. 여기 있는 이유가 그것이다 — 이 시점이 앱 전체에서
    /// `app.db` 를 아무도 잡고 있지 않은 유일한 지점이다.
    ///
    /// 옮기지 않고 복사만 한다. 원본은 수백 KB 라 남아도 문제가 없고, 새 위치가
    /// 잘못되었을 때 되돌아갈 자리가 된다.
    pub fn adopt_default_db(&self) -> Result<()> {
        let default = default_root()?;
        if self.root == default || self.db.exists() {
            return Ok(());
        }
        let source = default.join("app.db");
        if !source.is_file() {
            return Ok(());
        }
        // WAL 은 정상 종료 시 체크포인트되어 사라진다. 남아 있다면 비정상 종료라는
        // 뜻이므로 함께 가져와야 마지막 설정이 살아남는다.
        for suffix in ["", "-wal", "-shm"] {
            let from = with_suffix(&source, suffix);
            if from.is_file() {
                std::fs::copy(&from, with_suffix(&self.db, suffix))?;
            }
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

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    if suffix.is_empty() {
        return path.to_path_buf();
    }
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

// ================================================================ 루트 결정

/// 아무것도 설정하지 않았을 때의 위치. 포인터 파일도 항상 여기에 있다.
pub fn default_root() -> Result<PathBuf> {
    Ok(local_app_data()?.join(ROOT_DIR_NAME))
}

pub fn pointer_file() -> Result<PathBuf> {
    Ok(default_root()?.join(POINTER_FILE))
}

/// 환경변수 → 포인터 파일 → 기본값.
fn resolve_root() -> Result<PathBuf> {
    if let Ok(v) = std::env::var(ENV_OVERRIDE) {
        let v = v.trim();
        if !v.is_empty() {
            return Ok(PathBuf::from(v));
        }
    }

    let default = default_root()?;
    let pointer = std::fs::read_to_string(default.join(POINTER_FILE)).ok();
    Ok(root_from_pointer(default, pointer.as_deref()))
}

/// 포인터 파일의 내용으로 루트를 정한다.
///
/// **포인터가 없으면 기본 위치다.** 이 앱이 지금까지 만들어 온 모든 설치본이 그
/// 상태이고, 이 함수가 그들에게 하는 일은 문자 그대로 아무것도 없다 — 위치 변경
/// 기능 전체가 "파일 하나가 있을 때만" 켜진다.
///
/// 값이 이상하면 막지 말고 기본값으로 넘어진다. 여기서 에러를 내면 앱이 아예
/// 뜨지 않는데, 그때 사용자가 할 수 있는 일이 없다. 값이 멀쩡한데 드라이브만
/// 없는 경우는 `ensure_dirs()` 가 되돌리는 방법까지 담아 보고한다.
fn root_from_pointer(default: PathBuf, pointer: Option<&str>) -> PathBuf {
    match pointer.map(str::trim) {
        Some(s) if !s.is_empty() && Path::new(s).is_absolute() => PathBuf::from(s),
        _ => default,
    }
}

/// 포인터 파일을 쓴다. 기본 위치를 고르면 파일을 지운다 — 설정을 되돌린 흔적을
/// 남겨두면 다음 사람이 그 값을 진실로 믿는다.
///
/// **줄바꿈 없이 한 줄만 쓴다.** 언인스톨 훅이 `FileRead` 로 이 값을 그대로 읽어
/// 경로로 쓰기 때문이다.
pub fn write_pointer(root: &Path) -> Result<()> {
    let default = default_root()?;
    let file = pointer_file()?;
    if root == default {
        if file.exists() {
            std::fs::remove_file(&file)?;
        }
        return Ok(());
    }
    std::fs::create_dir_all(&default)?;
    std::fs::write(&file, root.to_string_lossy().as_bytes())?;
    Ok(())
}

// ================================================================ 위치 검사

/// 사용자가 고른 폴더를 실제 데이터 폴더 경로로 바꾼다.
///
/// 고른 폴더 **안에** `ChewieApp` 을 만든다. `D:\` 를 고르면 드라이브 루트에
/// `app.db` 와 `wsl\` 이 흩어지고, 그 상태에서 제거 훅이 도는 것은 생각만 해도
/// 아찔하다. 이미 `ChewieApp` 으로 끝나면 그대로 쓴다 (한 번 고른 폴더를 다시
/// 고르는 경우).
pub fn normalize_data_root(picked: &Path) -> Result<PathBuf> {
    validate_host_path(picked)?;
    if picked.file_name().and_then(|s| s.to_str()) == Some(ROOT_DIR_NAME) {
        return Ok(picked.to_path_buf());
    }
    Ok(picked.join(ROOT_DIR_NAME))
}

/// 대상 폴더가 데이터 폴더로 쓸 만한지 확인하고 최종 경로를 돌려준다.
///
/// 검사가 깐깐한 이유는 하나다 — 이 폴더는 제거 시 `RMDir /r` 로 통째로 사라진다.
/// 사용자의 다른 파일이 들어 있는 폴더를 고르게 두면 안 된다.
pub fn check_data_root(picked: &Path) -> Result<PathBuf> {
    let root = normalize_data_root(picked)?;

    if let Some(reason) = drive_problem(&root) {
        return Err(Error::InvalidInput(reason));
    }

    // 우리가 이미 쓰던 폴더는 통과. 그 외에 내용물이 있으면 거절한다.
    if root.exists() && !root.join("app.db").exists() {
        let mut entries = std::fs::read_dir(&root)?;
        if entries.next().is_some() {
            return Err(Error::InvalidInput(format!(
                "이미 다른 파일이 들어 있는 폴더입니다: {}\n\
                 앱을 제거할 때 이 폴더를 통째로 지우므로, 비어 있는 폴더나 새 폴더를 고르세요.",
                root.display()
            )));
        }
    }
    Ok(root)
}

/// 드라이브가 데이터 폴더를 감당하는지. 문제가 없으면 `None`.
///
/// `ext4.vhdx` 는 sparse 파일이라 NTFS/ReFS 가 아니면 만들어지지 않거나 처음부터
/// 전체 크기를 차지한다. 그리고 이동식 드라이브에 두면 그것을 뽑는 순간 배포판이
/// 통째로 사라진 것처럼 보인다 — 둘 다 설치 뒤에 알게 되면 늦는다.
#[cfg(windows)]
fn drive_problem(root: &Path) -> Option<String> {
    let drive = drive_letter(root)?;
    let script = format!(
        "$d = Get-CimInstance Win32_LogicalDisk -Filter \"DeviceID='{drive}'\" \
         -ErrorAction SilentlyContinue; \
         if ($d) {{ \"$($d.DriveType)|$($d.FileSystem)\" }} else {{ \"\" }}"
    );
    let out = crate::win::powershell(&script).ok()?;
    let text = out.stdout.trim().to_string();
    let (kind, fs) = text.split_once('|')?;

    // 2 = Removable, 4 = Network, 5 = CD-ROM. 3 = Local fixed 만 받는다.
    match kind.trim() {
        "3" => {}
        "2" => {
            return Some(format!(
                "{drive} 는 이동식 드라이브입니다. 드라이브를 뽑으면 배포판을 쓸 수 없게 되므로 \
                 내장 드라이브를 고르세요."
            ))
        }
        "4" => {
            return Some(format!(
                "{drive} 는 네트워크 드라이브입니다. 가상 디스크를 둘 수 없으므로 \
                 내장 드라이브를 고르세요."
            ))
        }
        // 판정하지 못한 경우는 막지 않는다 — 근거가 없는 것이지 문제가 있는 것이 아니다.
        _ => return None,
    }

    let fs = fs.trim().to_ascii_uppercase();
    if fs.is_empty() || fs == "NTFS" || fs == "REFS" {
        None
    } else {
        Some(format!(
            "{drive} 의 파일 시스템이 {fs} 입니다. 가상 디스크는 NTFS 또는 ReFS 에만 \
             둘 수 있습니다 (exFAT/FAT32 불가)."
        ))
    }
}

#[cfg(not(windows))]
fn drive_problem(_root: &Path) -> Option<String> {
    None
}

/// `D:\연구\ChewieApp` → `D:`. 드라이브 문자로 시작하지 않으면 `None`.
fn drive_letter(root: &Path) -> Option<String> {
    let s = root.to_string_lossy();
    let mut chars = s.chars();
    let letter = chars.next()?;
    if chars.next() != Some(':') || !letter.is_ascii_alphabetic() {
        return None;
    }
    Some(format!("{}:", letter.to_ascii_uppercase()))
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
    Err(Error::Other("%LOCALAPPDATA% 를 확인할 수 없습니다".into()))
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

    /// 드라이브 루트를 고르면 `app.db` 와 `wsl\` 이 드라이브에 흩어진다.
    /// 그 상태에서 언인스톨 훅이 도는 것을 막는 것이 이 규칙의 목적이다.
    #[test]
    fn a_picked_folder_always_gets_its_own_subdirectory() {
        assert_eq!(
            normalize_data_root(Path::new(r"D:\")).unwrap(),
            PathBuf::from(r"D:\ChewieApp")
        );
        assert_eq!(
            normalize_data_root(Path::new(r"D:\연구 자료")).unwrap(),
            PathBuf::from(r"D:\연구 자료\ChewieApp")
        );
    }

    /// 이미 고른 폴더를 다시 고르는 경우. `ChewieApp\ChewieApp` 이 되면 안 된다.
    #[test]
    fn an_already_normalized_path_is_left_alone() {
        assert_eq!(
            normalize_data_root(Path::new(r"D:\ChewieApp")).unwrap(),
            PathBuf::from(r"D:\ChewieApp")
        );
    }

    #[test]
    fn network_and_relative_targets_are_rejected() {
        assert!(normalize_data_root(Path::new(r"\\nas\share")).is_err());
        assert!(normalize_data_root(Path::new("data")).is_err());
    }

    /// 훅이 지우기 전에 확인하는 이름이다. 바꾸면 `nsis/hooks.nsh` 도 같이 고쳐야 한다.
    #[test]
    fn the_uninstall_hook_checks_this_directory_name() {
        let hook = include_str!("../nsis/hooks.nsh");
        assert!(hook.contains(ROOT_DIR_NAME));
        assert!(
            hook.contains(POINTER_FILE),
            "훅이 포인터 파일을 읽지 않으면 옮긴 폴더가 제거 후에도 남는다"
        );
    }

    #[test]
    fn drive_letter_is_extracted_from_a_windows_path() {
        assert_eq!(
            drive_letter(Path::new(r"d:\ChewieApp")).as_deref(),
            Some("D:")
        );
        assert_eq!(drive_letter(Path::new("/home/x")), None);
    }

    #[test]
    fn paths_hang_off_the_given_root() {
        let p = AppPaths::at(PathBuf::from(r"D:\ChewieApp"));
        assert_eq!(p.db, PathBuf::from(r"D:\ChewieApp\app.db"));
        assert_eq!(p.wsl, PathBuf::from(r"D:\ChewieApp\wsl"));
    }

    /// **기존 설치본 회귀 방지.** 0.4.2 까지 설치된 모든 기기에는 포인터 파일이
    /// 없다. 그 상태에서 루트가 조금이라도 달라지면 앱이 빈 DB 로 뜨고, 사용자는
    /// 작업 이력과 스키마 목록을 통째로 잃은 것처럼 보게 된다.
    #[test]
    fn an_existing_install_without_a_pointer_keeps_the_default_root() {
        let default = PathBuf::from(r"C:\Users\x\AppData\Local\ChewieApp");
        assert_eq!(root_from_pointer(default.clone(), None), default);
    }

    /// 포인터가 깨져 있어도 앱은 떠야 한다 — 판단이 애매하면 기본값 쪽으로 넘어진다.
    #[test]
    fn a_broken_pointer_falls_back_to_the_default_root() {
        let default = PathBuf::from(r"C:\Users\x\AppData\Local\ChewieApp");
        for junk in ["", "   ", "\n", "ChewieApp", "..\\..\\x"] {
            assert_eq!(
                root_from_pointer(default.clone(), Some(junk)),
                default,
                "포인터 {junk:?} 는 기본값으로 떨어져야 한다"
            );
        }
    }

    /// 앱이 쓴 값에는 줄바꿈이 없지만, 사용자가 메모장으로 열어 고칠 수도 있다.
    #[test]
    fn a_valid_pointer_wins_and_tolerates_trailing_newlines() {
        let default = PathBuf::from(r"C:\Users\x\AppData\Local\ChewieApp");
        assert_eq!(
            root_from_pointer(default, Some("D:\\ChewieApp\r\n")),
            PathBuf::from(r"D:\ChewieApp")
        );
    }

    #[test]
    fn wal_siblings_get_the_suffix_not_the_extension() {
        // `with_extension` 을 쓰면 `app.db-wal` 이 아니라 `app.-wal` 이 된다.
        assert_eq!(
            with_suffix(Path::new(r"D:\ChewieApp\app.db"), "-wal"),
            PathBuf::from(r"D:\ChewieApp\app.db-wal")
        );
    }
}
