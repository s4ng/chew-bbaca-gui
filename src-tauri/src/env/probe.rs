//! 환경 검사 (§7.3, §7.4).
//!
//! 게이트 순서를 코드로 그대로 옮긴 것이다. 각 게이트는 이전 게이트가 통과한
//! 경우에만 부작용을 일으킬 수 있다.
//!
//! ```text
//! wsl -d chewie-env -- true   (낙관적 시도)
//!   ├─ 성공 → Ready
//!   └─ 실패 → ① HypervisorPresent → ② wsl --status → ③ 배포판 없음
//! ```

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::win;

/// 다음에 사용자에게 보여줄 화면을 결정한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Gate {
    /// 바로 앱으로 진입한다. 질문 없음.
    Ready,
    /// 하드웨어 게이트에서 걸렸다. **아직 아무것도 바꾸지 않았다.**
    BiosVirtualization,
    /// WSL 자체가 없거나 기동하지 않는다. 설치 + 재부팅이 필요하다.
    WslMissing,
    /// WSL 은 정상. 전용 배포판만 없다. 다운로드 + import 로 끝난다.
    DistroMissing,
    /// 판정에 실패했다. 진단 정보를 그대로 보여주고 수동 안내로 넘긴다.
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvReport {
    pub gate: Gate,
    pub distro: String,
    pub distro_ready: bool,
    /// **1차 판정 기준** (§7.4)
    pub hypervisor_present: Option<bool>,
    /// 보조 신호. 단독으로 판정에 쓰지 않는다 — 하이퍼바이저가 이미 실행 중이면
    /// `False` 를 돌려주어 정상 기기를 오진한다.
    pub virtualization_firmware_enabled: Option<bool>,
    pub wsl_installed: bool,
    pub wsl_status_text: Option<String>,
    /// 사용자의 **기존** 배포판 목록. 표시만 하고 절대 건드리지 않는다.
    pub existing_distros: Vec<String>,
    /// BIOS 안내를 제조사별로 맞춤 표시하기 위한 값 (§7.6)
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    /// 진단 화면에 그대로 노출할 사람이 읽는 로그
    pub messages: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct HostFacts {
    #[serde(rename = "hypervisorPresent")]
    hypervisor_present: Option<bool>,
    #[serde(rename = "virtualizationFirmwareEnabled")]
    virtualization_firmware_enabled: Option<bool>,
    manufacturer: Option<String>,
    model: Option<String>,
}

/// §7.3 의 흐름을 그대로 실행한다. 부작용은 없다 — 읽기만 한다.
pub fn probe(distro: &str) -> Result<EnvReport> {
    let mut report = EnvReport {
        gate: Gate::Unknown,
        distro: distro.to_string(),
        distro_ready: false,
        hypervisor_present: None,
        virtualization_firmware_enabled: None,
        wsl_installed: false,
        wsl_status_text: None,
        existing_distros: Vec::new(),
        manufacturer: None,
        model: None,
        messages: Vec::new(),
    };

    // ── 낙관적 시도 ────────────────────────────────────────────────
    match try_distro(distro) {
        Ok(true) => {
            report.distro_ready = true;
            report.wsl_installed = true;
            report.gate = Gate::Ready;
            report.messages.push(format!("배포판 '{distro}' 정상 응답"));
            return Ok(report);
        }
        Ok(false) => report
            .messages
            .push(format!("배포판 '{distro}' 이(가) 응답하지 않습니다")),
        Err(e) => report.messages.push(format!("wsl.exe 실행 실패: {e}")),
    }

    // ── ① 하드웨어 게이트 ──────────────────────────────────────────
    let facts = host_facts();
    report.hypervisor_present = facts.hypervisor_present;
    report.virtualization_firmware_enabled = facts.virtualization_firmware_enabled;
    report.manufacturer = facts.manufacturer;
    report.model = facts.model;

    let verdict = hardware_verdict(
        report.hypervisor_present,
        report.virtualization_firmware_enabled,
    );
    match verdict {
        HardwareVerdict::Blocked => {
            report
                .messages
                .push("HypervisorPresent = False — 하이퍼바이저가 동작하지 않습니다".into());
            report.messages.push(
                "VirtualizationFirmwareEnabled 가 True 가 아닙니다 — 펌웨어에서 꺼져 있을 가능성이 큽니다"
                    .into(),
            );
            report.gate = Gate::BiosVirtualization;
            return Ok(report);
        }
        HardwareVerdict::Pending => {
            report.messages.push(
                "HypervisorPresent = False 이지만 펌웨어 가상화는 켜져 있습니다 — \
                 WSL 설치 전이라 하이퍼바이저가 아직 기동하지 않은 상태로 봅니다"
                    .into(),
            );
        }
        HardwareVerdict::Ok => {}
    }

    // ── ② WSL 게이트 ──────────────────────────────────────────────
    let status = wsl_status();
    report.wsl_installed = status.installed;
    report.wsl_status_text = status.text.clone();
    if !status.installed {
        report
            .messages
            .push("WSL 이 설치되어 있지 않거나 기동하지 않습니다".into());
        report.gate = Gate::WslMissing;
        return Ok(report);
    }

    // WSL 이 이미 있는데도 하이퍼바이저가 없다면 "아직 기동 전" 이라는 해명이 사라진다.
    // 여기서부터는 진짜 가상화 문제이므로 하드웨어 게이트로 되돌린다 — 이대로 ③ 으로
    // 보내면 `wsl --import` 가 0x80370102 로 죽고 사용자는 원인을 알 수 없다.
    if verdict == HardwareVerdict::Pending {
        report.messages.push(
            "WSL 은 설치되어 있는데 하이퍼바이저가 여전히 동작하지 않습니다 — \
             Windows 기능(Virtual Machine Platform)이나 부팅 설정을 확인해야 합니다"
                .into(),
        );
        report.gate = Gate::BiosVirtualization;
        return Ok(report);
    }

    // ── ③ 배포판 게이트 ────────────────────────────────────────────
    report.existing_distros = list_distros();
    report.messages.push(format!(
        "WSL 정상. 등록된 배포판: {}",
        if report.existing_distros.is_empty() {
            "(없음)".to_string()
        } else {
            report.existing_distros.join(", ")
        }
    ));
    report.gate = Gate::DistroMissing;
    Ok(report)
}

/// 게이트 ① 의 판정 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HardwareVerdict {
    /// 하이퍼바이저가 돌고 있다(또는 조회 실패). 다음 게이트로.
    Ok,
    /// 하이퍼바이저는 없지만 펌웨어 가상화는 켜져 있다 — 판정을 미룬다.
    Pending,
    /// 펌웨어에서 꺼져 있을 가능성이 크다. BIOS 안내로.
    Blocked,
}

/// 게이트 ① 을 시스템 호출 없이 판정한다.
///
/// `HypervisorPresent == false` 의 원인은 두 가지이고 처방이 완전히 다르다.
///
/// - 펌웨어에서 VT-x/SVM 이 꺼져 있다 → BIOS 를 켜야 한다
/// - Virtual Machine Platform 이 없어 하이퍼바이저가 애초에 기동하지 않았다 → WSL 설치면 된다
///
/// 후자는 WSL 을 한 번도 깔지 않은 기기의 **정상 상태**다. `HypervisorPresent` 만 보고
/// 막으면 BIOS 를 이미 켠 사용자가 [다시 검사] 를 아무리 눌러도 통과하지 못한다
/// (실제 신고: LG gram, 펌웨어 True / 하이퍼바이저 False / WSL 미설치).
/// 그래서 펌웨어가 `True` 면 판정을 게이트 ② 이후로 미룬다.
///
/// 반대로 `VirtualizationFirmwareEnabled` 를 **단독**으로 믿어서도 안 된다 —
/// 하이퍼바이저가 이미 돌고 있으면 `False` 를 돌려주므로, 이 값은 여기서처럼
/// `HypervisorPresent == false` 인 경우에 한해 본다.
fn hardware_verdict(
    hypervisor_present: Option<bool>,
    firmware_enabled: Option<bool>,
) -> HardwareVerdict {
    if hypervisor_present != Some(false) {
        return HardwareVerdict::Ok;
    }
    if firmware_enabled == Some(true) {
        HardwareVerdict::Pending
    } else {
        HardwareVerdict::Blocked
    }
}

/// 낙관적 시도. `Ok(false)` 는 "wsl 은 있는데 배포판이 없다" 는 뜻이다.
fn try_distro(distro: &str) -> Result<bool> {
    let mut cmd = win::command("wsl.exe");
    cmd.env("WSL_UTF8", "1");
    cmd.args(["-d", distro, "-e", "true"]);
    let out = win::capture(&mut cmd)?;
    Ok(out.ok())
}

struct WslStatus {
    installed: bool,
    text: Option<String>,
}

fn wsl_status() -> WslStatus {
    let mut cmd = win::command("wsl.exe");
    cmd.env("WSL_UTF8", "1");
    cmd.arg("--status");
    match win::capture(&mut cmd) {
        Ok(out) => {
            let text = format!("{}{}", out.stdout, out.stderr).trim().to_string();
            WslStatus {
                installed: out.ok(),
                text: if text.is_empty() { None } else { Some(text) },
            }
        }
        // wsl.exe 자체가 없다 (Windows 기능 미활성화).
        Err(e) => WslStatus {
            installed: false,
            text: Some(e.to_string()),
        },
    }
}

/// 전용 배포판이 이미 등록되어 있는지.
///
/// `try_distro()` 와 달리 VM 을 깨우지 않아 즉시 끝난다. "데이터 폴더를 아직
/// 옮겨도 되는가" 처럼 화면을 그릴 때마다 물어야 하는 곳에서 쓴다.
pub fn distro_registered(distro: &str) -> bool {
    list_distros()
        .iter()
        .any(|d| d.eq_ignore_ascii_case(distro))
}

/// 사용자의 기존 배포판 목록. **표시 전용**이다 — 원칙 "사용자 환경 불가침".
fn list_distros() -> Vec<String> {
    let mut cmd = win::command("wsl.exe");
    cmd.env("WSL_UTF8", "1");
    cmd.args(["--list", "--quiet"]);
    match win::capture(&mut cmd) {
        Ok(out) if out.ok() => out
            .stdout
            .lines()
            .map(|l| l.trim().trim_end_matches("(기본값)").trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// CIM 두 개를 한 번의 PowerShell 호출로 모은다. 실패해도 진단을 계속할 수
/// 있도록 개별 필드는 `Option` 이다.
fn host_facts() -> HostFacts {
    let script = r#"
$cs  = Get-CimInstance Win32_ComputerSystem -ErrorAction SilentlyContinue
$cpu = Get-CimInstance Win32_Processor -ErrorAction SilentlyContinue | Select-Object -First 1
[pscustomobject]@{
  hypervisorPresent             = if ($cs)  { [bool]$cs.HypervisorPresent } else { $null }
  virtualizationFirmwareEnabled = if ($cpu) { [bool]$cpu.VirtualizationFirmwareEnabled } else { $null }
  manufacturer                  = if ($cs)  { $cs.Manufacturer } else { $null }
  model                         = if ($cs)  { $cs.Model } else { $null }
} | ConvertTo-Json -Compress
"#;
    match win::powershell(script) {
        Ok(out) if out.ok() => serde_json::from_str(out.stdout.trim()).unwrap_or_default(),
        _ => HostFacts::default(),
    }
}

/// 제조사 문자열 → BIOS 진입 안내 (§7.6).
///
/// 설정 항목 이름은 제조사마다 다르다. 여기 없는 제조사는 일반 안내로 떨어진다.
pub fn firmware_hint(manufacturer: Option<&str>) -> (&'static str, &'static str) {
    let m = manufacturer.unwrap_or("").to_ascii_lowercase();
    if m.contains("lenovo") {
        (
            "F1 또는 Enter → F1",
            "Security → Virtualization → Intel VT-x / AMD-V",
        )
    } else if m.contains("dell") {
        ("F2", "Virtualization Support → Virtualization")
    } else if m.contains("hp") || m.contains("hewlett") {
        (
            "F10 (일부 기종 Esc → F10)",
            "Security → System Security → Virtualization Technology",
        )
    } else if m.contains("asus") {
        (
            "F2 또는 Del",
            "Advanced → CPU Configuration → Intel Virtualization Technology / SVM Mode",
        )
    } else if m.contains("gigabyte") {
        ("Del", "M.I.T. → Advanced Frequency → SVM Mode / Intel VT-x")
    } else if m.contains("msi") {
        (
            "Del",
            "OC → CPU Features → Intel Virtualization Tech / SVM Mode",
        )
    } else if m.contains("samsung") {
        ("F2", "Advanced → Virtualization Technology")
    } else if m.contains("lg electronics") || m == "lg" {
        // gram 계열은 부팅 로고에서 F2. 항목이 Advanced 안에 숨어 있는 기종이 있다.
        ("F2", "Advanced → Intel Virtualization Technology (VT-x)")
    } else if m.contains("acer") {
        ("F2", "Main 또는 Advanced → Virtualization Technology")
    } else {
        (
            "F2 / Del / F10 (기종에 따라 다름)",
            "Advanced 또는 CPU Configuration 하위의 Intel Virtualization Technology(VT-x) 또는 SVM Mode",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 하이퍼바이저가 돌고 있으면 펌웨어 값이 무엇이든 통과한다 — `False` 로
    /// 보고되는 것이 이 상황의 정상이다.
    #[test]
    fn running_hypervisor_passes_regardless_of_firmware_flag() {
        assert_eq!(
            hardware_verdict(Some(true), Some(false)),
            HardwareVerdict::Ok
        );
        assert_eq!(hardware_verdict(Some(true), None), HardwareVerdict::Ok);
    }

    /// 신고 사례: BIOS 에서 VT-x 를 켰는데 앱이 계속 BIOS 화면을 보여줬다.
    /// 펌웨어가 켜져 있으면 여기서 막지 않는다.
    #[test]
    fn firmware_on_but_hypervisor_down_is_pending_not_blocked() {
        assert_eq!(
            hardware_verdict(Some(false), Some(true)),
            HardwareVerdict::Pending
        );
    }

    #[test]
    fn firmware_off_or_unknown_blocks_at_bios_gate() {
        assert_eq!(
            hardware_verdict(Some(false), Some(false)),
            HardwareVerdict::Blocked
        );
        assert_eq!(
            hardware_verdict(Some(false), None),
            HardwareVerdict::Blocked
        );
    }

    /// CIM 조회 자체가 실패했을 때 하드웨어 게이트에서 막으면 안 된다 —
    /// 판정 근거가 없는 것이지 가상화가 꺼진 것이 아니다.
    #[test]
    fn unknown_hypervisor_falls_through() {
        assert_eq!(hardware_verdict(None, None), HardwareVerdict::Ok);
    }

    #[test]
    fn firmware_hint_falls_back_for_unknown_vendor() {
        let (key, _) = firmware_hint(Some("Some OEM"));
        assert!(key.contains("F2"));
    }

    #[test]
    fn firmware_hint_knows_lg() {
        let (_, path) = firmware_hint(Some("LG Electronics"));
        assert!(path.contains("Advanced"));
    }

    #[test]
    fn firmware_hint_is_case_insensitive() {
        let (key, _) = firmware_hint(Some("LENOVO"));
        assert!(key.starts_with("F1"));
    }
}
