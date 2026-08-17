//! 배포판 설치·검증·제거·업데이트 (§4.3, §7.5, §8.3).
//!
//! 여기서 일어나는 일은 전부 **사용자 환경 불가침** 원칙 아래에 있다.
//! 우리가 소유하는 것은 `chewie-env` 배포판 하나와 `%LOCALAPPDATA%\ChewieApp`
//! 뿐이다. `.wslconfig` 는 전역 설정이므로 읽지도 쓰지도 않는다.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::paths::AppPaths;
use crate::win;

/// rootfs 배포 정보. 릴리스마다 값이 바뀌므로 설정으로 뺀다 (§8.1).
///
/// `url` 은 **덮어쓰기 수단**이다. 비어 있는 것이 정상이며, 그때는 인스톨러에
/// 동봉된 rootfs 를 쓴다. 직접 빌드한 이미지를 시험할 때만 채운다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootfsSource {
    pub url: String,
    /// 소문자 16진 SHA256. 불일치 시 받은 파일을 폐기한다.
    pub sha256: String,
    pub file_name: String,
    pub version: String,
}

/// rootfs 를 어디서 가져오는지. UI 문구가 이 값으로 갈린다 —
/// 동봉본은 "내려받는다" 고 말하면 안 되고, 없으면 버튼을 주면 안 된다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RootfsOrigin {
    /// 인스톨러에 함께 담긴 파일. 일반 사용자는 항상 이 경로다.
    Bundled,
    /// 설정에 적힌 로컬 tar.gz (직접 빌드 / 사설망 배포)
    LocalFile,
    /// 설정에 적힌 http(s) 주소
    Remote,
    /// 어느 쪽도 없다 — 자동 설치를 시도하면 안 된다.
    Missing,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub received: u64,
    /// 서버가 Content-Length 를 주지 않으면 `None`
    pub total: Option<u64>,
}

/// 동봉 rootfs 가 놓이는 리소스 하위 폴더 (`tauri.bundle.json` 의 매핑 대상과 같아야 한다).
const BUNDLED_SUBDIR: &str = "rootfs";

pub struct Provisioner {
    paths: AppPaths,
    distro: String,
    /// Tauri 리소스 디렉터리. 개발 실행에서는 리소스가 복사되지 않으므로
    /// 경로는 있어도 파일이 없다 — 존재 여부로만 판단한다.
    resources: Option<PathBuf>,
}

impl Provisioner {
    pub fn new(paths: AppPaths, distro: impl Into<String>) -> Self {
        Self {
            paths,
            distro: distro.into(),
            resources: None,
        }
    }

    pub fn with_resources(mut self, dir: Option<PathBuf>) -> Self {
        self.resources = dir;
        self
    }

    // ------------------------------------------------------------ WSL 설치

    /// `wsl --install --no-distribution` 을 권한 상승 헬퍼로 대행한다 (§7.5).
    ///
    /// `--no-distribution` 은 필수다. 빼면 Ubuntu 가 함께 설치되어
    /// "사용자 환경 불가침" 원칙을 정면으로 위반한다.
    ///
    /// 실패 경로가 둘 있다.
    /// * UAC 거부 → `ElevationDenied`. 호출자는 **명령어 복사 안내**로 폴백한다.
    /// * Microsoft Store 차단(기업 장비) → `--inbox` 로 한 번 더 시도한다.
    pub fn install_wsl(&self) -> Result<String> {
        let first = win::run_elevated("wsl.exe", &["--install", "--no-distribution"])?;
        if first.ok() {
            return Ok("WSL 설치 요청이 완료되었습니다. 재부팅 후 다시 실행하세요.".into());
        }

        // Store 가 차단된 환경에서는 인박스 버전으로 우회한다.
        let inbox = win::run_elevated("wsl.exe", &["--install", "--no-distribution", "--inbox"])?;
        if inbox.ok() {
            return Ok(
                "WSL(인박스 버전) 설치 요청이 완료되었습니다. 재부팅 후 다시 실행하세요.".into(),
            );
        }

        Err(Error::Other(format!(
            "WSL 설치에 실패했습니다 (exit {}). 관리자 PowerShell 에서 직접 실행해 주세요.\n{}",
            inbox.code,
            inbox.stderr.trim()
        )))
    }

    /// WSL1 이 기본값인 경우를 정정한다 (§7.5-4). 권한 상승이 필요 없다.
    pub fn set_default_version_2(&self) -> Result<()> {
        let mut cmd = win::command("wsl.exe");
        cmd.env("WSL_UTF8", "1")
            .args(["--set-default-version", "2"]);
        win::capture(&mut cmd)?;
        Ok(())
    }

    /// 권한 상승이 거부되었을 때 보여줄 수동 경로. 버튼을 제공하되
    /// **수동 경로를 없애지 않는다**(§7.5-3).
    pub fn manual_commands() -> Vec<&'static str> {
        vec![
            "wsl --install --no-distribution",
            "wsl --set-default-version 2",
            "wsl --update --web-download",
        ]
    }

    /// UEFI 펌웨어로 바로 재부팅한다 (§7.6-1).
    ///
    /// "부팅 중 F2 연타" 장벽을 없애는 것이 목적이다. 레거시 BIOS 기기에서는
    /// 동작하지 않으므로 호출자는 항상 수동 안내를 함께 보여야 한다.
    /// 재부팅을 유발하므로 **UI 에서 명시적으로 한 번 더 확인**한 뒤 호출한다.
    pub fn reboot_to_firmware(&self) -> Result<()> {
        win::run_elevated("shutdown.exe", &["/r", "/fw", "/t", "5"])?;
        Ok(())
    }

    // ------------------------------------------------------------ rootfs

    /// 인스톨러에 동봉된 rootfs 의 경로. 파일이 실제로 있을 때만 `Some`.
    pub fn bundled_rootfs(&self, source: &RootfsSource) -> Option<PathBuf> {
        let path = self
            .resources
            .as_ref()?
            .join(BUNDLED_SUBDIR)
            .join(&source.file_name);
        path.is_file().then_some(path)
    }

    /// 지금 이 설정으로 rootfs 를 어디서 가져오게 되는지 (§8.1).
    ///
    /// 설정의 `url` 이 동봉본보다 우선한다. 동봉본을 두고도 다른 이미지를
    /// 시험하려는 것이 URL 을 채우는 유일한 이유이기 때문이다.
    pub fn rootfs_origin(&self, source: &RootfsSource) -> RootfsOrigin {
        if !source.url.trim().is_empty() {
            return match local_source_path(&source.url) {
                Some(_) => RootfsOrigin::LocalFile,
                None => RootfsOrigin::Remote,
            };
        }
        match self.bundled_rootfs(source) {
            Some(_) => RootfsOrigin::Bundled,
            None => RootfsOrigin::Missing,
        }
    }

    /// rootfs 를 확보하고 SHA256 을 검증해 import 할 파일 경로를 돌려준다.
    ///
    /// 동봉본과 로컬 파일은 **제자리에서 해싱만** 한다. 500MB 를 캐시로 한 번 더
    /// 복사할 이유가 없다. 원격만 실제 다운로드를 타며, 그때는 앱이 죽거나 네트워크가
    /// 끊길 수 있으므로 `.part` 로 받은 뒤 검증에 성공했을 때만 최종 이름으로 옮긴다.
    pub fn download_rootfs(
        &self,
        source: &RootfsSource,
        on_progress: &dyn Fn(DownloadProgress),
    ) -> Result<PathBuf> {
        self.paths.ensure_dirs()?;

        // 여기서 panic 하면 진행 이벤트를 못 보낸 채 스레드가 죽어 UI 가 영영 멈춘다.
        // 불변조건이 성립하더라도 `?` 로만 빠져나간다.
        match self.rootfs_origin(source) {
            RootfsOrigin::Bundled => {
                let bundled = self.bundled_rootfs(source).ok_or_else(|| missing(source))?;
                return verify_local(&bundled, source, on_progress);
            }
            RootfsOrigin::LocalFile => {
                let local = local_source_path(&source.url).ok_or_else(|| missing(source))?;
                return verify_local(&local, source, on_progress);
            }
            RootfsOrigin::Missing => return Err(missing(source)),
            RootfsOrigin::Remote => {}
        }

        let target = self.paths.rootfs_cache(&source.file_name);

        if target.exists() {
            let total = std::fs::metadata(&target).ok().map(|m| m.len());
            let actual = sha256_file(&target, &mut |received| {
                on_progress(DownloadProgress { received, total })
            })?;
            if actual.eq_ignore_ascii_case(&source.sha256) {
                return Ok(target);
            }
            // 손상된 캐시는 조용히 버린다.
            let _ = std::fs::remove_file(&target);
        }

        let part = target.with_extension("part");
        let resp = ureq::get(&source.url)
            .call()
            .map_err(|e| Error::Download(e.to_string()))?;

        let total = resp
            .header("Content-Length")
            .and_then(|v| v.parse::<u64>().ok());

        let mut reader = resp.into_reader();
        let mut file = std::fs::File::create(&part)?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 256 * 1024];
        let mut received: u64 = 0;
        let mut last_report: u64 = 0;

        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            hasher.update(&buf[..n]);
            received += n as u64;
            // 4MB 마다 보고한다. 매 청크마다 이벤트를 쏘면 UI 가 밀린다.
            if received - last_report >= 4 * 1024 * 1024 {
                last_report = received;
                on_progress(DownloadProgress { received, total });
            }
        }
        file.flush()?;
        drop(file);
        on_progress(DownloadProgress { received, total });

        let actual = hex(&hasher.finalize());
        if !actual.eq_ignore_ascii_case(&source.sha256) {
            let _ = std::fs::remove_file(&part);
            return Err(Error::ChecksumMismatch {
                expected: source.sha256.clone(),
                actual,
            });
        }

        std::fs::rename(&part, &target)?;
        Ok(target)
    }

    /// `wsl --import` 로 전용 배포판을 등록한다 (§8.3).
    pub fn import_distro(&self, tarball: &Path) -> Result<()> {
        self.paths.ensure_dirs()?;
        // 배열 원소 타입을 &str 로 통일한다 (Cow/String 을 섞으면 컴파일되지 않는다).
        let install_dir = self.paths.wsl.to_string_lossy().to_string();
        let tar = tarball.to_string_lossy().to_string();

        let mut cmd = win::command("wsl.exe");
        cmd.env("WSL_UTF8", "1").args([
            "--import",
            self.distro.as_str(),
            install_dir.as_str(),
            tar.as_str(),
            "--version",
            "2",
        ]);
        win::capture(&mut cmd)?.require_success()?;
        Ok(())
    }

    /// 제거는 한 줄로 끝난다 — 전용 배포판을 따로 등록한 가장 큰 이유다(§8.3).
    pub fn unregister(&self) -> Result<()> {
        let mut cmd = win::command("wsl.exe");
        cmd.env("WSL_UTF8", "1")
            .args(["--unregister", self.distro.as_str()]);
        win::capture(&mut cmd)?.require_success()?;
        Ok(())
    }

    // ------------------------------------------------------------ 디스크

    /// `ext4.vhdx` 는 파일을 지워도 자동으로 줄지 않는다 (§6.5).
    /// 압축 전에 반드시 배포판을 종료해야 한다.
    pub fn compact_disk(&self) -> Result<String> {
        let mut term = win::command("wsl.exe");
        term.env("WSL_UTF8", "1")
            .args(["--terminate", self.distro.as_str()]);
        let _ = win::capture(&mut term);

        let mut cmd = win::command("wsl.exe");
        cmd.env("WSL_UTF8", "1")
            .args(["--manage", self.distro.as_str(), "--set-sparse", "true"]);
        let out = win::capture(&mut cmd)?;
        if out.ok() {
            // **"여유 공간이 반환됩니다" 라고 단정하지 않는다.** sparse 는 지연 반납이라
            // 누른 직후의 파일 크기는 대개 그대로이고, 그렇게 쓰면 사용자는 정리가
            // 실패했다고 읽는다. 실제로 얼마가 줄었는지는 호출부가 전후를 비교해 붙인다.
            return Ok("가상 디스크를 sparse 모드로 전환했습니다.".into());
        }
        Err(Error::Other(format!(
            "디스크 정리에 실패했습니다. WSL 버전이 --manage 를 지원하지 않을 수 있습니다.\n{}",
            out.stderr.trim()
        )))
    }

    /// vhdx 실제 크기 (바이트). 설정 화면의 "디스크 사용량" 표시에 쓴다.
    pub fn vhdx_size(&self) -> Option<u64> {
        let vhdx = self.paths.wsl.join("ext4.vhdx");
        std::fs::metadata(vhdx).ok().map(|m| m.len())
    }
}

/// `url` 이 http(s) 가 아니면 로컬 파일 소스로 본다 (`file://` 접두사도 허용).
///
/// 동봉본 대신 직접 빌드한 `rootfs/build.sh` 산출물을 시험할 때 쓰는 경로다.
fn local_source_path(url: &str) -> Option<PathBuf> {
    let u = url.trim();
    if u.is_empty() || u.starts_with("http://") || u.starts_with("https://") {
        return None;
    }
    // `file:///C:/...` → `C:/...`, `file://C:/...` → `C:/...`
    let raw = u
        .strip_prefix("file:///")
        .or_else(|| u.strip_prefix("file://"))
        .unwrap_or(u);
    Some(PathBuf::from(raw))
}

/// 로컬 tar.gz 를 해싱해 검증한다. 해싱만으로도 500MB 는 수 초 걸리므로
/// 다운로드와 같은 진행률 이벤트를 흘려 UI 가 멈춘 것처럼 보이지 않게 한다.
fn verify_local(
    path: &Path,
    source: &RootfsSource,
    on_progress: &dyn Fn(DownloadProgress),
) -> Result<PathBuf> {
    if !path.is_file() {
        return Err(Error::Download(format!(
            "rootfs 파일을 찾을 수 없습니다: {}\n설정 화면의 [rootfs 이미지] 칸에 올바른 tar.gz 경로를 넣거나, 칸을 비워 인스톨러 동봉본을 쓰세요.",
            path.display()
        )));
    }
    let total = std::fs::metadata(path).ok().map(|m| m.len());
    let actual = sha256_file(path, &mut |received| {
        on_progress(DownloadProgress { received, total })
    })?;
    if !actual.eq_ignore_ascii_case(&source.sha256) {
        return Err(Error::ChecksumMismatch {
            expected: source.sha256.clone(),
            actual,
        });
    }
    Ok(path.to_path_buf())
}

/// 가져올 곳이 아무 데도 없을 때. 인스톨러 사용자와 개발자의 다음 행동이 다르므로
/// 두 경로를 모두 적는다.
fn missing(source: &RootfsSource) -> Error {
    Error::Download(format!(
        "설치할 rootfs 이미지를 찾을 수 없습니다: {}\n인스톨러로 설치한 앱이라면 다시 설치해 주세요. 개발 중이라면 설정 화면의 [rootfs 이미지] 칸에 직접 빌드한 tar.gz 경로를 넣으세요.",
        source.file_name
    ))
}

fn sha256_file(path: &Path, on_read: &mut dyn FnMut(u64)) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut read_total: u64 = 0;
    let mut last_report: u64 = 0;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        read_total += n as u64;
        // 다운로드와 같은 주기(4MB)로 보고한다. 매 청크마다 쏘면 UI 가 밀린다.
        if read_total - last_report >= 4 * 1024 * 1024 {
            last_report = read_total;
            on_read(read_total);
        }
    }
    on_read(read_total);
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_encodes_lowercase() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff]), "000fff");
    }

    #[test]
    fn http_urls_are_not_treated_as_local_files() {
        assert!(local_source_path("https://example.com/rootfs.tar.gz").is_none());
        assert!(local_source_path("").is_none());
    }

    #[test]
    fn windows_paths_and_file_urls_resolve_to_local_files() {
        assert_eq!(
            local_source_path(r"C:\dist-rootfs\chewie-rootfs-3.5.4.tar.gz"),
            Some(PathBuf::from(r"C:\dist-rootfs\chewie-rootfs-3.5.4.tar.gz"))
        );
        assert_eq!(
            local_source_path("file:///C:/dist-rootfs/chewie-rootfs-3.5.4.tar.gz"),
            Some(PathBuf::from("C:/dist-rootfs/chewie-rootfs-3.5.4.tar.gz"))
        );
    }

    fn source(url: &str) -> RootfsSource {
        RootfsSource {
            url: url.into(),
            sha256: "0".repeat(64),
            file_name: "chewie-rootfs-3.5.4.tar.gz".into(),
            version: "3.5.4".into(),
        }
    }

    fn provisioner(resources: Option<PathBuf>) -> Provisioner {
        Provisioner::new(AppPaths::resolve().unwrap(), "chewie-env").with_resources(resources)
    }

    #[test]
    fn origin_is_missing_without_a_bundled_file_or_url() {
        // 개발 실행이 정확히 이 상태다 — 리소스가 복사되지 않는다.
        assert_eq!(
            provisioner(None).rootfs_origin(&source("")),
            RootfsOrigin::Missing
        );
    }

    #[test]
    fn origin_is_bundled_when_the_resource_file_exists() {
        let dir = std::env::temp_dir().join("chewie-origin-test");
        let rootfs_dir = dir.join(BUNDLED_SUBDIR);
        std::fs::create_dir_all(&rootfs_dir).unwrap();
        std::fs::write(rootfs_dir.join("chewie-rootfs-3.5.4.tar.gz"), b"x").unwrap();

        assert_eq!(
            provisioner(Some(dir.clone())).rootfs_origin(&source("")),
            RootfsOrigin::Bundled
        );

        // 설정의 URL 은 동봉본을 이긴다 — 직접 빌드한 이미지를 시험하는 유일한 수단이다.
        assert_eq!(
            provisioner(Some(dir.clone())).rootfs_origin(&source(r"C:\tmp\other.tar.gz")),
            RootfsOrigin::LocalFile
        );
        assert_eq!(
            provisioner(Some(dir.clone())).rootfs_origin(&source("https://example.com/r.tar.gz")),
            RootfsOrigin::Remote
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bundled_subdir_matches_the_bundle_config() {
        // `src-tauri/tauri.bundle.json` 의 매핑 대상 경로와 어긋나면 동봉본을 못 찾는다.
        let cfg = include_str!("../../tauri.bundle.json");
        assert!(cfg.contains(&format!("\"{BUNDLED_SUBDIR}/")));
    }

    #[test]
    fn manual_commands_keep_no_distribution_flag() {
        // 이 플래그가 빠지면 Ubuntu 가 함께 깔린다 — 회귀 방지.
        assert!(Provisioner::manual_commands()[0].contains("--no-distribution"));
    }
}
