# Interactive BLXCode setup for Windows development and local desktop builds.
$ErrorActionPreference = "Stop"

$Yes = $false
$CheckOnly = $false
$SkipSystem = $false
$NoVerify = $false
$WithBundle = $false
$Warnings = New-Object System.Collections.Generic.List[string]

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Root = Split-Path -Parent (Split-Path -Parent $ScriptDir)

function Show-Usage {
    @"
Usage: powershell -ExecutionPolicy Bypass -File scripts/setup-windows.ps1 [options]

Set up BLXCode after a git clone on Windows.

Options:
  --yes          Accept install prompts.
  --check-only   Inspect and print planned actions without installing or building.
  --skip-system  Skip winget/system prerequisite installs.
  --no-verify    Install/check prerequisites but skip cargo/trunk verification.
  --with-bundle  Run cargo tauri build after verification.
  -h, --help     Show this help.
"@
}

foreach ($Arg in $args) {
    switch ($Arg) {
        "--yes" { $Yes = $true; continue }
        "--check-only" { $CheckOnly = $true; continue }
        "--skip-system" { $SkipSystem = $true; continue }
        "--no-verify" { $NoVerify = $true; continue }
        "--with-bundle" { $WithBundle = $true; continue }
        "-h" { Show-Usage; exit 0 }
        "--help" { Show-Usage; exit 0 }
        default { throw "Unknown option: $Arg (use --help)" }
    }
}

function Write-Section([string]$Message) {
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Info([string]$Message) {
    Write-Host "info  $Message" -ForegroundColor Blue
}

function Write-Ok([string]$Message) {
    Write-Host "ok    $Message" -ForegroundColor Green
}

function Write-WarningLine([string]$Message) {
    [void]$script:Warnings.Add($Message)
    Write-Host "warn  $Message" -ForegroundColor Yellow
}

function Confirm-Step([string]$Prompt) {
    if ($script:Yes) {
        return $true
    }

    try {
        $Answer = Read-Host "$Prompt [y/N]"
    } catch {
        Write-WarningLine "Skipping prompt in non-interactive shell: $Prompt"
        return $false
    }

    return $Answer -match "^(y|yes)$"
}

function Test-Command([string]$Name) {
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Invoke-Step([string]$Description, [scriptblock]$Action) {
    Write-Host "  + $Description" -ForegroundColor DarkGray
    if ($script:CheckOnly) {
        return
    }

    & $Action
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed: $Description"
    }
}

function Add-CargoBinToPath {
    $CargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if (($env:Path -split ";") -notcontains $CargoBin) {
        $env:Path = "$CargoBin;$env:Path"
    }
}

function Install-WingetPackage([string]$Id, [string]$Name, [string]$Override = "") {
    if ($script:SkipSystem) {
        Write-WarningLine "Skipping $Name install (--skip-system)."
        return
    }
    if (-not (Test-Command "winget")) {
        Write-WarningLine "winget is not available. Install $Name manually and re-run."
        return
    }
    if ($script:CheckOnly) {
        $Planned = "winget install --id $Id -e --accept-package-agreements --accept-source-agreements"
        if ($Override) {
            $Planned = "$Planned --override `"$Override`""
        }
        Write-Info "Would offer: $Planned"
        return
    }
    if (-not (Confirm-Step "Install $Name with winget?")) {
        Write-WarningLine "$Name install skipped."
        return
    }

    $WingetArgs = @(
        "install",
        "--id", $Id,
        "-e",
        "--accept-package-agreements",
        "--accept-source-agreements"
    )
    if ($Override) {
        $WingetArgs += @("--override", $Override)
    }

    Invoke-Step "winget $($WingetArgs -join ' ')" {
        & winget @WingetArgs
    }
}

function Test-WingetInstalled([string]$Id) {
    if (-not (Test-Command "winget")) {
        return $false
    }

    & winget list --id $Id -e *> $null
    return $LASTEXITCODE -eq 0
}

function Test-NativeCppWorkload {
    $ProgramFilesX86 = ${env:ProgramFiles(x86)}
    if (-not $ProgramFilesX86) {
        return $false
    }

    $VsWhere = Join-Path $ProgramFilesX86 "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $VsWhere)) {
        return $false
    }

    $InstallPath = & $VsWhere -products * -requires Microsoft.VisualStudio.Workload.NativeDesktop -property installationPath 2>$null | Select-Object -First 1
    return -not [string]::IsNullOrWhiteSpace($InstallPath)
}

function Ensure-SystemDependencies {
    Write-Section "System dependencies"

    if ($SkipSystem) {
        Write-WarningLine "Skipping winget/system setup (--skip-system)."
        Write-Info "Required manually: Microsoft C++ Build Tools with Desktop development with C++, WebView2 Runtime, Rustup, Node.js LTS."
        return
    }

    if (-not (Test-Command "rustup") -or -not (Test-Command "cargo")) {
        Install-WingetPackage "Rustlang.Rustup" "Rustup"
        Add-CargoBinToPath
    } else {
        Write-Ok "Rustup/Cargo are available."
    }

    if (-not (Test-Command "node") -or -not (Test-Command "npm")) {
        Install-WingetPackage "OpenJS.NodeJS.LTS" "Node.js LTS"
    } else {
        Write-Ok "Node/npm are available."
    }

    if (-not (Test-WingetInstalled "Microsoft.EdgeWebView2Runtime")) {
        Install-WingetPackage "Microsoft.EdgeWebView2Runtime" "Microsoft Edge WebView2 Runtime"
    } else {
        Write-Ok "Microsoft Edge WebView2 Runtime appears to be installed."
    }

    if (-not (Test-NativeCppWorkload)) {
        Install-WingetPackage `
            "Microsoft.VisualStudio.2022.BuildTools" `
            "Microsoft C++ Build Tools" `
            "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.NativeDesktop --includeRecommended"
    } else {
        Write-Ok "Microsoft C++ native desktop workload appears to be installed."
    }

    Write-WarningLine "MSI bundling may require the Windows VBSCRIPT optional feature because Tauri bundle targets are set to all."
}

function Ensure-Rust {
    Write-Section "Rust toolchain"
    Add-CargoBinToPath

    if (-not (Test-Command "rustup") -or -not (Test-Command "cargo")) {
        Write-WarningLine "Rust/Cargo was not found in PATH. Restart this terminal after installing Rustup, then re-run this script."
        return
    }

    Invoke-Step "rustup default stable-msvc" {
        & rustup default stable-msvc
    }
    Invoke-Step "rustup target add wasm32-unknown-unknown" {
        & rustup target add wasm32-unknown-unknown
    }
    Write-Ok "Rust is available: $(& cargo --version)"
}

function Get-NodeMajorVersion {
    if (-not (Test-Command "node")) {
        return 0
    }
    $Major = & node -p "Number(process.versions.node.split('.')[0])" 2>$null
    if ($LASTEXITCODE -ne 0 -or -not $Major) {
        return 0
    }
    return [int]$Major
}

function Ensure-Node {
    Write-Section "Node and npm"

    if (-not (Test-Command "node") -or -not (Test-Command "npm")) {
        Write-WarningLine "Node.js and npm are required for frontend-js. Install Node.js LTS, preferably 22, then re-run."
        return
    }

    $Major = Get-NodeMajorVersion
    if ($Major -lt 18) {
        throw "Node.js >= 18 is required; found $(& node --version). Install Node.js LTS, preferably 22."
    }

    Write-Ok "Node is available: $(& node --version) (Node 22 recommended)"
    Write-Ok "npm is available: $(& npm --version)"
}

function Test-CargoTauri {
    if (-not (Test-Command "cargo")) {
        return $false
    }
    & cmd /c "cargo tauri --version >NUL 2>NUL"
    return $LASTEXITCODE -eq 0
}

function Ensure-CargoTools {
    Write-Section "Cargo tools"

    if (-not (Test-Command "cargo")) {
        Write-WarningLine "Skipping Cargo tool setup because cargo is not available."
        return
    }

    if (Test-Command "trunk") {
        Write-Ok "Trunk is available: $(& trunk --version)"
    } else {
        Invoke-Step "cargo install trunk --locked" {
            & cargo install trunk --locked
        }
        Add-CargoBinToPath
    }

    if (Test-CargoTauri) {
        Write-Ok "Cargo Tauri CLI is available: $(& cargo tauri --version)"
    } else {
        Invoke-Step 'cargo install tauri-cli --version "^2" --locked' {
            & cargo install tauri-cli --version "^2" --locked
        }
        Add-CargoBinToPath
    }
}

function Run-FrontendSetup {
    Write-Section "Frontend JavaScript dependencies"

    if (-not (Test-Command "npm")) {
        Write-WarningLine "Skipping npm setup because npm is not available."
        return
    }

    Invoke-Step "npm ci --prefix frontend-js" {
        & npm ci --prefix (Join-Path $script:Root "frontend-js")
    }
    Invoke-Step "npm --prefix frontend-js run build:graph3d" {
        & npm --prefix (Join-Path $script:Root "frontend-js") run build:graph3d
    }
}

function Run-Verification {
    Write-Section "Verification"

    if ($NoVerify) {
        Write-WarningLine "Skipping verification (--no-verify)."
        return
    }

    Push-Location $Root
    try {
        Invoke-Step "cargo check -p blxcode" {
            & cargo check -p blxcode
        }
        Invoke-Step "cargo check -p blxcode-ui --target wasm32-unknown-unknown" {
            & cargo check -p blxcode-ui --target wasm32-unknown-unknown
        }
        Invoke-Step "cargo test --workspace" {
            & cargo test --workspace
        }
        Invoke-Step "trunk build" {
            & trunk build
        }
        if ($WithBundle) {
            Write-Section "Bundle build"
            Invoke-Step "cargo tauri build" {
                & cargo tauri build
            }
        }
    } finally {
        Pop-Location
    }
}

function Show-Summary {
    Write-Section "Summary"
    Write-Info "Repo root: $Root"
    if ($CheckOnly) {
        Write-Info "Mode: check-only; no installs or builds were executed."
    }
    if ($Warnings.Count -gt 0) {
        Write-Host "warn  Completed with $($Warnings.Count) warning(s):" -ForegroundColor Yellow
        foreach ($Item in $Warnings) {
            Write-Host "  - $Item"
        }
    } else {
        Write-Ok "Setup completed without warnings."
    }
}

Write-Section "BLXCode Windows setup"
Write-Info "This script sets up Rust/MSVC, Tauri, Trunk, Node/npm, WebView2, and Windows native dependencies."
Write-Info "Default verification does not launch the app. Use --with-bundle for cargo tauri build."

Ensure-SystemDependencies
Ensure-Rust
Ensure-Node
Ensure-CargoTools
Run-FrontendSetup
Run-Verification
Show-Summary
