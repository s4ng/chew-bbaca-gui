@echo off
REM ===================================================================
REM  chewBBACA Desktop GUI - environment check (temporary, single file)
REM
REM  Batch header below launches PowerShell, which re-reads this same
REM  file and executes everything after the marker line below.
REM  Double-click to run. Options:  check-env.bat -NoElevate
REM
REM  NOTE: this file must keep CRLF line endings and must NOT have a
REM        UTF-8 BOM. The PowerShell part is read as UTF-8 explicitly.
REM ===================================================================
setlocal
chcp 65001 >nul
title chewBBACA - env check
set "SELF=%~f0"
set "ARGS=%*"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$t=[IO.File]::ReadAllText($env:SELF,[Text.Encoding]::UTF8);$m='#'+'PSBEGIN';$i=$t.IndexOf($m);Invoke-Expression $t.Substring($i+$m.Length)"
endlocal
exit /b

#PSBEGIN
# =====================================================================
#  chewBBACA Desktop GUI - 환경 사전 점검
#
#  ARCHITECTURE.md 7.3 의 게이트 순서를 그대로 따라간다.
#      (1) 하드웨어 가상화 -> (2) Windows 기능 -> (3) WSL -> (4) 배포판
#
#  관리자 권한이 필요한 항목:
#      - Get-WindowsOptionalFeature (VirtualMachinePlatform / WSL 기능)
#      - bcdedit (hypervisorlaunchtype)
#  권한 상승이 거부되면 해당 항목만 [SKIP] 하고 나머지는 계속 검사한다.
# =====================================================================

$ErrorActionPreference = 'Continue'
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }

$Self      = $env:SELF
$NoElevate = ($env:ARGS -match '(?i)no-?elevate')

# ------------------------------------------------------------------ 권한 상승

function Test-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $pr = New-Object Security.Principal.WindowsPrincipal($id)
    return $pr.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

$IsAdminNow = Test-Admin

if (-not $IsAdminNow -and -not $NoElevate) {
    if ([string]::IsNullOrEmpty($Self) -or -not (Test-Path -LiteralPath $Self)) {
        Write-Host '  스크립트 경로를 알 수 없어 권한 상승을 건너뜁니다.' -ForegroundColor DarkYellow
    }
    else {
        Write-Host ''
        Write-Host '  일부 항목(Windows 기능, 부팅 설정)은 관리자 권한이 필요합니다.' -ForegroundColor Yellow
        Write-Host '  권한 상승을 요청합니다. UAC 창에서 [예]를 선택하세요.' -ForegroundColor Yellow
        try {
            # cmd 는 인자를 직접 인용해 주지 않으면 공백 경로에서 끊긴다.
            # (예: C:\Users\홍 길동\Desktop\check-env.bat)
            $selfQuoted = '"' + $Self + '"'
            Start-Process -FilePath 'cmd.exe' -Verb RunAs -ErrorAction Stop `
                -ArgumentList @('/c', $selfQuoted)
            exit 0
        }
        catch {
            Write-Host ''
            Write-Host '  권한 상승이 거부되었습니다.' -ForegroundColor DarkYellow
            Write-Host '  관리자 전용 항목은 [SKIP]으로 표시하고 나머지를 계속 검사합니다.' -ForegroundColor DarkYellow
        }
    }
    $IsAdminNow = Test-Admin
}

# ------------------------------------------------------------------ 출력 헬퍼

$script:Actions = New-Object System.Collections.Generic.List[string]

function Get-DisplayWidth {
    # 한글/CJK 는 콘솔에서 2칸을 차지하지만 .NET 문자열 길이로는 1이다.
    param([string]$Text)
    $w = 0
    foreach ($ch in $Text.ToCharArray()) {
        $c = [int]$ch
        if (($c -ge 0x1100 -and $c -le 0x115F) -or ($c -ge 0x2E80 -and $c -le 0xA4CF) -or
            ($c -ge 0xAC00 -and $c -le 0xD7A3) -or ($c -ge 0xF900 -and $c -le 0xFAFF) -or
            ($c -ge 0xFE30 -and $c -le 0xFE6F) -or ($c -ge 0xFF00 -and $c -le 0xFF60) -or
            ($c -ge 0xFFE0 -and $c -le 0xFFE6)) { $w += 2 }
        else { $w += 1 }
    }
    return $w
}

function Write-Section {
    param([string]$Title)
    Write-Host ''
    Write-Host ('  ' + $Title) -ForegroundColor Cyan
    Write-Host ('  ' + ('-' * 66)) -ForegroundColor DarkGray
}

function Write-Item {
    param(
        [string]$Label,
        [string]$Value,
        [ValidateSet('OK', 'FAIL', 'WARN', 'INFO', 'SKIP')]
        [string]$Status = 'INFO'
    )
    switch ($Status) {
        'OK'    { $color = 'Green';    $tag = '[ OK ]' }
        'FAIL'  { $color = 'Red';      $tag = '[FAIL]' }
        'WARN'  { $color = 'Yellow';   $tag = '[WARN]' }
        'SKIP'  { $color = 'DarkGray'; $tag = '[SKIP]' }
        default { $color = 'Gray';     $tag = '[INFO]' }
    }
    $pad = [Math]::Max(1, 38 - (Get-DisplayWidth $Label))
    Write-Host ('  {0} ' -f $tag) -ForegroundColor $color -NoNewline
    Write-Host ($Label + (' ' * $pad)) -NoNewline
    Write-Host $Value -ForegroundColor $color
}

function Add-Action {
    param([string]$Text)
    $script:Actions.Add($Text) | Out-Null
}

function Invoke-Wsl {
    # WSL_UTF8=1 을 설정하지 않으면 출력이 UTF-16LE 로 나와 파싱이 깨진다 (6.4)
    param([string[]]$WslArgs)
    $prev = $env:WSL_UTF8
    $env:WSL_UTF8 = '1'
    $out = ''
    try   { $out = (& wsl.exe @WslArgs 2>&1 | Out-String) }
    catch { $out = '' }
    finally {
        if ($null -eq $prev) { Remove-Item Env:\WSL_UTF8 -ErrorAction SilentlyContinue }
        else { $env:WSL_UTF8 = $prev }
    }
    # 구버전 WSL 은 WSL_UTF8 을 무시하므로 널 바이트를 방어적으로 제거한다
    return ($out -replace "`0", '').Trim()
}

Write-Host ''
Write-Host '  ==================================================================' -ForegroundColor White
Write-Host '   chewBBACA Desktop GUI - 환경 사전 점검' -ForegroundColor White
Write-Host '  ==================================================================' -ForegroundColor White
if ($IsAdminNow) { Write-Item '실행 권한' '관리자' 'OK' }
else { Write-Item '실행 권한' '일반 사용자 (일부 항목 제한)' 'WARN' }

# ------------------------------------------------------------------ 1. 시스템

Write-Section '1. 시스템'

$os  = Get-CimInstance Win32_OperatingSystem -ErrorAction SilentlyContinue
$cs  = Get-CimInstance Win32_ComputerSystem -ErrorAction SilentlyContinue
$cpu = @(Get-CimInstance Win32_Processor -ErrorAction SilentlyContinue)[0]

$build = 0
if ($os) {
    $build = [int]$os.BuildNumber
    Write-Item 'OS' ('{0} (build {1})' -f $os.Caption, $os.BuildNumber) 'INFO'
}

if ($build -ge 19041) {
    Write-Item '빌드 요구사항' 'wsl --install 사용 가능 (19041+)' 'OK'
}
elseif ($build -ge 18362) {
    Write-Item '빌드 요구사항' 'WSL2 가능하나 wsl --install 불가 (수동 설치)' 'WARN'
    Add-Action 'Windows 를 21H2 이상으로 업데이트하면 설치가 단순해집니다.'
}
elseif ($build -gt 0) {
    Write-Item '빌드 요구사항' 'WSL2 최소 빌드(18362) 미달' 'FAIL'
    Add-Action 'Windows 업데이트가 필수입니다. 현재 빌드에서는 WSL2 를 쓸 수 없습니다.'
}

if ($cs) {
    Write-Item '제조사 / 모델' ('{0} / {1}' -f $cs.Manufacturer, $cs.Model) 'INFO'
    if ($cs.Model -match 'Virtual|VMware|KVM|Xen|VirtualBox') {
        Write-Item '가상 머신 여부' '가상 머신으로 보임 - 중첩 가상화 필요' 'WARN'
        Add-Action '이 PC 가 가상 머신이면 호스트에서 중첩 가상화를 노출해야 합니다.'
    }
}
if ($cpu) { Write-Item 'CPU' $cpu.Name 'INFO' }

# ------------------------------------------- 2. 하드웨어 가상화 (게이트 1)

Write-Section '2. 하드웨어 가상화  [ARCHITECTURE.md 7.3 - 게이트 1]'

$hypervisorPresent = $false
if ($cs -and $null -ne $cs.HypervisorPresent) { $hypervisorPresent = [bool]$cs.HypervisorPresent }

$firmwareVirt = $null; $slat = $null; $vmx = $null
if ($cpu) {
    $firmwareVirt = $cpu.VirtualizationFirmwareEnabled
    $slat         = $cpu.SecondLevelAddressTranslationExtensions
    $vmx          = $cpu.VMMonitorModeExtensions
}

if ($hypervisorPresent) { Write-Item 'HypervisorPresent' 'True - 하이퍼바이저 동작 중' 'OK' }
else { Write-Item 'HypervisorPresent' 'False - 하이퍼바이저 미동작' 'FAIL' }

# 아래 세 값은 하이퍼바이저가 CPU 를 점유하면 조회가 불가능해져 False 로 보고된다.
# HypervisorPresent=True 인 상황에서는 판정에 쓰지 않고 참고용으로만 표시한다.
$cpuPropsTrusted = -not $hypervisorPresent

function Format-CpuProp {
    param($Value)
    if ($cpuPropsTrusted) { return [string]$Value }
    if ($Value -eq $false) { return 'False  (조회 불가로 인한 값 - 무시할 것)' }
    return ('{0}  (참고용)' -f $Value)
}

Write-Item 'VirtualizationFirmwareEnabled' (Format-CpuProp $firmwareVirt) 'INFO'
Write-Item 'VMMonitorModeExtensions' (Format-CpuProp $vmx) 'INFO'
Write-Item 'SLAT (EPT/NPT)' (Format-CpuProp $slat) 'INFO'

if (-not $cpuPropsTrusted) {
    Write-Host ''
    Write-Host '   위 세 값이 False 여도 문제가 아닙니다. 하이퍼바이저가 이미 CPU 를 점유하면' -ForegroundColor DarkGray
    Write-Host '   호스트 OS 는 펌웨어 상태를 조회할 수 없습니다. HypervisorPresent=True 가' -ForegroundColor DarkGray
    Write-Host '   가상화 동작의 확정적 근거이며, 이 값들로 판정하면 오진입니다.' -ForegroundColor DarkGray
}

# 게이트 1 판정 - HypervisorPresent 를 1차 기준으로 삼는다
if ($hypervisorPresent)          { $gate1 = 'PASS' }
elseif ($firmwareVirt -eq $true) { $gate1 = 'PASS-PENDING' }
else                             { $gate1 = 'BIOS-SUSPECT' }

# ----------------------------------- 3. Windows 기능 / 부팅 설정 (게이트 2)

Write-Section '3. Windows 기능 및 부팅 설정  [게이트 2]'

$featWsl = $null; $featVmp = $null

if ($IsAdminNow) {
    foreach ($f in @(
            @{ Name = 'Microsoft-Windows-Subsystem-Linux'; Label = 'WSL 기능';                Var = 'featWsl' },
            @{ Name = 'VirtualMachinePlatform';            Label = 'Virtual Machine Platform'; Var = 'featVmp' },
            @{ Name = 'Microsoft-Hyper-V';                 Label = 'Hyper-V (선택)';           Var = 'featHv'  }
        )) {
        try { $state = (Get-WindowsOptionalFeature -Online -FeatureName $f.Name -ErrorAction Stop).State }
        catch { $state = 'Unknown' }
        Set-Variable -Name $f.Var -Value $state -Scope Script
        if ($state -eq 'Enabled') { Write-Item $f.Label 'Enabled' 'OK' }
        elseif ($f.Name -eq 'Microsoft-Hyper-V') { Write-Item $f.Label ([string]$state) 'INFO' }
        else { Write-Item $f.Label ([string]$state) 'FAIL' }
    }

    $bcd = ''
    try { $bcd = (& bcdedit.exe /enum '{current}' 2>&1 | Out-String) } catch { }
    if ($bcd -match 'hypervisorlaunchtype\s+(\w+)') {
        $hlt = $Matches[1]
        if ($hlt -match '^Off$') {
            Write-Item 'hypervisorlaunchtype' 'Off - 하이퍼바이저 부팅 차단됨' 'FAIL'
            Add-Action '관리자 권한으로 실행:  bcdedit /set hypervisorlaunchtype auto   (재부팅 필요)'
        }
        else { Write-Item 'hypervisorlaunchtype' $hlt 'OK' }
    }
    else { Write-Item 'hypervisorlaunchtype' '미지정 (기본값 Auto)' 'OK' }
}
else {
    Write-Item 'WSL / VMP 기능 상태' '관리자 권한 필요 - 건너뜀' 'SKIP'
    Write-Item 'hypervisorlaunchtype' '관리자 권한 필요 - 건너뜀' 'SKIP'
}

# --------------------------------------------------- 4. WSL 상태 (게이트 3)

Write-Section '4. WSL  [게이트 3]'

$wslCmd = Get-Command wsl.exe -ErrorAction SilentlyContinue
$wslWorks = $false
$wslStatus = ''

if (-not $wslCmd) {
    Write-Item 'wsl.exe' '없음' 'FAIL'
}
else {
    Write-Item 'wsl.exe' $wslCmd.Source 'OK'

    $appx = Get-AppxPackage -Name 'MicrosoftCorporationII.WindowsSubsystemForLinux' -ErrorAction SilentlyContinue
    if ($appx) { Write-Item 'WSL 배포 형태' ('Store 패키지 v{0}' -f $appx.Version) 'INFO' }
    else { Write-Item 'WSL 배포 형태' '인박스(Windows 기능) 또는 미설치' 'INFO' }

    $wslStatus = Invoke-Wsl @('--status')
    if ($LASTEXITCODE -eq 0 -and $wslStatus.Length -gt 0) {
        $wslWorks = $true
        foreach ($line in ($wslStatus -split "`r?`n")) {
            if ($line.Trim().Length -gt 0) { Write-Host ('         {0}' -f $line.Trim()) -ForegroundColor Gray }
        }
    }
    else {
        Write-Item 'wsl --status' ('실패 (exit {0})' -f $LASTEXITCODE) 'FAIL'
        if ($wslStatus.Length -gt 0) {
            Write-Host ('         {0}' -f ($wslStatus -split "`r?`n")[0]) -ForegroundColor DarkGray
        }
        Add-Action '관리자 권한으로 실행:  wsl --install --no-distribution   (재부팅 필요)'
    }

    $ver = Invoke-Wsl @('--version')
    if ($LASTEXITCODE -eq 0 -and $ver -match 'WSL') {
        foreach ($line in ($ver -split "`r?`n")) {
            if ($line -match '커널|kernel|WSL 버전|WSL version') {
                Write-Host ('         {0}' -f $line.Trim()) -ForegroundColor Gray
            }
        }
    }

    $list = Invoke-Wsl @('--list', '--verbose')
    if ($LASTEXITCODE -eq 0 -and $list.Length -gt 0) {
        Write-Host ''
        Write-Item '등록된 배포판' '' 'INFO'
        foreach ($line in ($list -split "`r?`n")) {
            if ($line.Trim().Length -gt 0) { Write-Host ('         {0}' -f $line.TrimEnd()) -ForegroundColor Gray }
        }
        if ($list -match 'chewie-env') {
            Write-Item 'chewie-env' '등록됨' 'OK'
            $null = Invoke-Wsl @('-d', 'chewie-env', '--', 'true')
            if ($LASTEXITCODE -eq 0) { Write-Item 'chewie-env 실행' '정상 (낙관적 시도 통과)' 'OK' }
            else { Write-Item 'chewie-env 실행' ('실패 (exit {0})' -f $LASTEXITCODE) 'FAIL' }
        }
        else {
            Write-Item 'chewie-env' '없음 - rootfs import 필요' 'WARN'
            Add-Action 'chewie-env 배포판이 없습니다. rootfs 를 받아 등록하세요:'
            Add-Action '  wsl --import chewie-env "%LOCALAPPDATA%\ChewieApp\wsl" chewie-rootfs.tar.gz --version 2'
        }
    }
    else {
        Write-Item '등록된 배포판' '없음 또는 조회 실패' 'WARN'
    }
}

# ------------------------------------------------------------------ 5. 결론

Write-Section '5. 결론'

if ($gate1 -eq 'PASS') {
    Write-Host '   [게이트 1] 하드웨어 가상화 : 통과' -ForegroundColor Green
}
elseif ($gate1 -eq 'PASS-PENDING') {
    Write-Host '   [게이트 1] 하드웨어 가상화 : 펌웨어는 켜져 있으나 하이퍼바이저 미기동' -ForegroundColor Yellow
    Add-Action 'Windows 기능(VirtualMachinePlatform, WSL)을 활성화하고 재부팅하세요.'
}
else {
    Write-Host '   [게이트 1] 하드웨어 가상화 : BIOS/UEFI 에서 꺼져 있을 가능성이 높습니다' -ForegroundColor Red
    Write-Host ''
    Write-Host '     확인 방법 : 작업 관리자 > 성능 > CPU > "가상화" 항목' -ForegroundColor Gray
    Write-Host '                 여기서 "사용 안 함" 이면 BIOS 문제가 확정입니다.' -ForegroundColor Gray
    if ($cs) { Write-Host ('     제조사    : {0}' -f $cs.Manufacturer) -ForegroundColor Gray }
    Write-Host '     설정 이름 : Intel = Intel Virtualization Technology / VT-x' -ForegroundColor Gray
    Write-Host '                 AMD   = SVM Mode' -ForegroundColor Gray
    Write-Host '     위치      : Advanced / CPU Configuration / M.I.T. / OC Tweaker 등' -ForegroundColor Gray
    Add-Action 'UEFI 로 바로 재부팅하려면 관리자 권한으로:  shutdown /r /fw /t 1'
    Add-Action 'BIOS 에서 가상화를 켠 뒤 이 스크립트를 다시 실행하세요.'
}

if ($IsAdminNow) {
    if ($featVmp -eq 'Enabled' -and $featWsl -eq 'Enabled') {
        Write-Host '   [게이트 2] Windows 기능    : 통과' -ForegroundColor Green
    }
    else {
        Write-Host '   [게이트 2] Windows 기능    : 미활성화' -ForegroundColor Red
        if ($gate1 -eq 'PASS' -or $gate1 -eq 'PASS-PENDING') {
            if ($build -ge 19041) {
                Add-Action '관리자 권한으로 실행:  wsl --install --no-distribution   (재부팅 필요)'
                Add-Action 'Store 가 차단된 환경이면:  wsl --install --no-distribution --inbox'
            }
            else {
                Add-Action '관리자 권한으로 DISM 두 줄을 실행한 뒤 재부팅하세요:'
                Add-Action '  dism /online /enable-feature /featurename:Microsoft-Windows-Subsystem-Linux /all /norestart'
                Add-Action '  dism /online /enable-feature /featurename:VirtualMachinePlatform /all /norestart'
            }
        }
        else {
            Write-Host '              (게이트 1 을 먼저 해결하세요. 지금 켜도 VM 기동은 실패합니다.)' -ForegroundColor DarkYellow
        }
    }
}
else {
    Write-Host '   [게이트 2] Windows 기능    : 확인 불가 (관리자 권한 없음)' -ForegroundColor DarkGray
}

if ($wslWorks) { Write-Host '   [게이트 3] WSL             : 통과' -ForegroundColor Green }
else { Write-Host '   [게이트 3] WSL             : 미설치 또는 비정상' -ForegroundColor Red }

if ($wslWorks -and $wslStatus -match '(기본 버전|Default Version)\s*:\s*1') {
    Add-Action '기본 버전이 WSL1 입니다. 실행:  wsl --set-default-version 2'
}

Write-Host ''
if ($script:Actions.Count -eq 0) {
    Write-Host '   조치 필요 항목이 없습니다.' -ForegroundColor Green
}
else {
    Write-Host '   다음 조치:' -ForegroundColor Yellow
    $i = 1
    foreach ($a in $script:Actions) {
        if ($a.StartsWith('  ')) { Write-Host ('       {0}' -f $a.Trim()) -ForegroundColor Gray }
        else {
            Write-Host ('     {0}. {1}' -f $i, $a) -ForegroundColor Gray
            $i++
        }
    }
}

Write-Host ''
Write-Host '  ==================================================================' -ForegroundColor White
Write-Host ''

if ($Host.Name -eq 'ConsoleHost' -and -not [Console]::IsInputRedirected) {
    Read-Host '  Enter 키를 누르면 종료합니다' | Out-Null
}
