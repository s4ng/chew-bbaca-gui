//! Windows 프로세스 기동 헬퍼.
//!
//! 이 모듈이 존재하는 이유는 단 두 가지 세부사항 때문이다 (§6.4).
//!
//! * `CREATE_NO_WINDOW` — 지정하지 않으면 `wsl.exe`/`powershell.exe` 를 호출할
//!   때마다 검은 콘솔 창이 깜빡인다. 40분짜리 작업 중 수십 번 반복되면
//!   사용자는 앱이 고장 났다고 판단한다.
//! * `WSL_UTF8=1` — 미설정 시 `wsl --status` 등이 UTF-16LE 를 출력해 파싱이 깨진다.

use std::process::{Command, Stdio};

use crate::error::{Error, Result};

/// `CREATE_NO_WINDOW` (winbase.h). 상수 하나를 위해 winapi 를 끌어오지 않는다.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 콘솔 창을 띄우지 않는 `Command` 를 만든다. 자식 프로세스는 항상 이 함수를 거친다.
pub fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// 실행 결과를 문자열로 모아 돌려준다.
pub struct Captured {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Captured {
    pub fn ok(&self) -> bool {
        self.code == 0
    }

    /// 0 이 아닌 종료 코드를 에러로 승격한다.
    pub fn require_success(self) -> Result<Self> {
        if self.ok() {
            Ok(self)
        } else {
            Err(Error::WslCommand {
                code: self.code,
                stderr: if self.stderr.trim().is_empty() {
                    self.stdout.trim().to_string()
                } else {
                    self.stderr.trim().to_string()
                },
            })
        }
    }
}

pub fn capture(cmd: &mut Command) -> Result<Captured> {
    let out = cmd.stdin(Stdio::null()).output()?;
    Ok(Captured {
        code: out.status.code().unwrap_or(-1),
        stdout: decode(&out.stdout),
        stderr: decode(&out.stderr),
    })
}

/// UTF-8 우선. `WSL_UTF8=1` 을 설정해도 일부 경로(예: WSL 미설치 시 shim)에서는
/// UTF-16LE 이 나올 수 있어 BOM/널바이트 패턴으로 한 번 더 방어한다.
pub fn decode(bytes: &[u8]) -> String {
    if looks_like_utf16le(bytes) {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units)
            .trim_start_matches('\u{feff}')
            .to_string();
    }
    String::from_utf8_lossy(bytes)
        .trim_start_matches('\u{feff}')
        .to_string()
}

fn looks_like_utf16le(bytes: &[u8]) -> bool {
    if bytes.len() < 4 || bytes.len() % 2 != 0 {
        return false;
    }
    if bytes[0] == 0xFF && bytes[1] == 0xFE {
        return true;
    }
    // ASCII 문자가 UTF-16LE 로 들어오면 홀수 바이트가 0 으로 채워진다.
    let sample = bytes.len().min(64);
    let zeros = bytes[..sample]
        .iter()
        .skip(1)
        .step_by(2)
        .filter(|b| **b == 0)
        .count();
    zeros * 2 > sample / 2
}

/// PowerShell 을 비대화형으로 실행하고 stdout 을 돌려준다.
///
/// 출력 인코딩을 UTF-8 로 고정한다 — 그렇지 않으면 파이프로 나가는 문자열이
/// 콘솔 코드페이지(예: CP949)를 따라가 한글 제조사명이 깨진다.
pub fn powershell(script: &str) -> Result<Captured> {
    let wrapped = format!(
        "try {{ [Console]::OutputEncoding = [Text.Encoding]::UTF8 }} catch {{ }}\r\n{script}"
    );
    capture(
        command("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command"])
            .arg(wrapped),
    )
}

/// UAC 로 권한 상승된 자식 프로세스를 기동하고 종료를 기다린다.
///
/// 앱 본체는 계속 비관리자로 남는다 (§7.5). 사용자가 UAC 에서 [아니오] 를 누르면
/// `Start-Process` 가 예외를 던지므로 `ElevationDenied` 로 변환해 호출자가
/// **수동 명령 안내 폴백**으로 내려갈 수 있게 한다.
pub fn run_elevated(program: &str, args: &[&str]) -> Result<Captured> {
    let arg_list = if args.is_empty() {
        String::new()
    } else {
        let quoted: Vec<String> = args.iter().map(|a| format!("'{}'", a.replace('\'', "''"))).collect();
        format!(" -ArgumentList @({})", quoted.join(","))
    };

    let script = format!(
        r#"try {{
  $p = Start-Process -FilePath '{program}'{arg_list} -Verb RunAs -Wait -PassThru -WindowStyle Hidden
  exit $p.ExitCode
}} catch {{
  Write-Error $_.Exception.Message
  exit 1223
}}"#
    );

    let out = powershell(&script)?;
    // 1223 = ERROR_CANCELLED. UAC 취소와 정책 차단이 모두 여기로 온다.
    if out.code == 1223 || out.stderr.contains("취소") || out.stderr.contains("cancel") {
        return Err(Error::ElevationDenied);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf16le_output() {
        // `wsl --list` 가 WSL_UTF8 없이 내보내는 형태.
        let bytes: Vec<u8> = "Ubuntu".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(decode(&bytes), "Ubuntu");
    }

    #[test]
    fn decodes_plain_utf8() {
        assert_eq!(decode("배포판".as_bytes()), "배포판");
    }
}
