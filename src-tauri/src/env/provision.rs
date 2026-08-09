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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootfsSource {
    pub url: String,
    /// 소문자 16진 SHA256. 불일치 시 받은 파일을 폐기한다.
    pub sha256: String,
    pub file_name: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub received: u64,
    /// 서버가 Content-Length 를 주지 않으면 `None`
    pub total: Option<u64>,
}

pub struct Provisioner {
    paths: AppPaths,
    distro: String,
}

impl Provisioner {
    pub fn new(paths: AppPaths, distro: impl Into<String>) -> Self {
        Self {
            paths,
            distro: distro.into(),
        }
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
            return Ok("WSL(인박스 버전) 설치 요청이 완료되었습니다. 재부팅 후 다시 실행하세요.".into());
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
        cmd.env("WSL_UTF8", "1").args(["--set-default-version", "2"]);
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

    /// rootfs 를 캐시로 내려받고 SHA256 을 검증한다.
    ///
    /// 400~800MB 를 받는 동안 앱이 죽거나 네트워크가 끊길 수 있으므로
    /// `.part` 로 받은 뒤 검증에 성공했을 때만 최종 이름으로 옮긴다.
    /// 이미 검증된 파일이 있으면 다시 받지 않는다.
    pub fn download_rootfs(
        &self,
        source: &RootfsSource,
        on_progress: &dyn Fn(DownloadProgress),
    ) -> Result<PathBuf> {
        self.paths.ensure_dirs()?;
        let target = self.paths.rootfs_cache(&source.file_name);

        if target.exists() {
            let actual = sha256_file(&target)?;
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
            return Ok("가상 디스크를 sparse 모드로 전환했습니다. 여유 공간이 반환됩니다.".into());
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

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
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
    fn manual_commands_keep_no_distribution_flag() {
        // 이 플래그가 빠지면 Ubuntu 가 함께 깔린다 — 회귀 방지.
        assert!(Provisioner::manual_commands()[0].contains("--no-distribution"));
    }
}
