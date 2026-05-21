# Shared helpers for scripts/release.ps1 - dot-source only.

function Initialize-ReleaseCommon {
    param([Parameter(Mandatory = $true)][string]$ReleaseScriptDir)

    $script:RELEASE_SCRIPT_DIR = (Resolve-Path $ReleaseScriptDir).Path
    $script:RELEASE_ROOT = (Resolve-Path (Join-Path $script:RELEASE_SCRIPT_DIR "..\..")).Path
    Set-Location $script:RELEASE_ROOT
    $script:RELEASE_GIT_REMOTE = Get-ReleaseGitRemote
    $script:RELEASE_GH_REPO = Get-ReleaseGitHubRepo
}

function Get-ReleaseGitRemote {
    if ($env:RELEASE_GIT_REMOTE_OVERRIDE) {
        return $env:RELEASE_GIT_REMOTE_OVERRIDE
    }

    $upstream = (& git rev-parse --abbrev-ref --symbolic-full-name "@{u}" 2>$null)
    if ($LASTEXITCODE -eq 0 -and $upstream -match "/") {
        return ($upstream -split "/", 2)[0]
    }

    return "origin"
}

function Get-ReleaseGitHubRepo {
    if ($env:RELEASE_GH_REPO_OVERRIDE) {
        return $env:RELEASE_GH_REPO_OVERRIDE
    }

    $remote = Get-ReleaseGitRemote
    $url = (& git remote get-url $remote 2>$null)
    if ($LASTEXITCODE -ne 0 -or -not $url) {
        throw "cannot read git remote URL for '$remote'"
    }

    if ($url -notmatch "github\.com[:/]([^/]+)/(.+?)(?:\.git)?$") {
        throw "cannot parse GitHub repo from remote URL: '$url'"
    }

    return "$($Matches[1])/$($Matches[2])"
}

function Write-ReleaseLog {
    param(
        [Parameter(Mandatory = $true)][string]$Level,
        [Parameter(Mandatory = $true)][string]$Message
    )
    Write-Host "[release:$Level] $Message" -ForegroundColor $(
        switch ($Level) {
            "info" { "Cyan" }
            "warn" { "Yellow" }
            "error" { "Red" }
            default { "White" }
        }
    )
}

function Write-ReleaseInfo { param([string]$Message) Write-ReleaseLog "info" $Message }
function Write-ReleaseWarn { param([string]$Message) Write-ReleaseLog "warn" $Message }
function Write-ReleaseError { param([string]$Message) Write-ReleaseLog "error" $Message }

function Stop-Release {
    param([string]$Message)
    Write-ReleaseError $Message
    exit $script:RELEASE_EXIT_USER
}

function Get-ReleaseTauriConfPath {
    return (Join-Path $script:RELEASE_ROOT "src-tauri\tauri.conf.json")
}

function Get-ReleaseChangelogPath {
    return (Join-Path $script:RELEASE_ROOT "CHANGELOG.md")
}

function Read-ReleaseVersion {
    $conf = Get-ReleaseTauriConfPath
    $text = [System.IO.File]::ReadAllText($conf, [System.Text.UTF8Encoding]::new($false, $true))
    return ($text | ConvertFrom-Json).version
}

function Get-ReleasePlatform {
    if ($script:RELEASE_PLATFORM_OVERRIDE) {
        return $script:RELEASE_PLATFORM_OVERRIDE
    }

    if ($IsWindows -or $env:OS -eq "Windows_NT") {
        return "windows"
    }
    if ($IsMacOS) {
        return "macos"
    }
    if ($IsLinux) {
        return "linux"
    }

    Stop-Release "Unsupported OS for release.ps1"
}

function Import-ReleaseEnv {
    $envFile = Join-Path $script:RELEASE_ROOT ".env.release"
    if (-not (Test-Path $envFile)) {
        return
    }

    Write-ReleaseInfo "Loading $envFile"
    foreach ($line in Get-Content $envFile) {
        $trimmed = $line.Trim()
        if (-not $trimmed -or $trimmed.StartsWith("#") -or $trimmed -notmatch "^([A-Za-z_][A-Za-z0-9_]*)=(.*)$") {
            continue
        }

        $key = $Matches[1]
        $value = $Matches[2].Trim()
        if (($value.StartsWith('"') -and $value.EndsWith('"')) -or ($value.StartsWith("'") -and $value.EndsWith("'"))) {
            $value = $value.Substring(1, $value.Length - 2)
        }
        Set-Item -Path "Env:$key" -Value $value
    }
}

function Test-ReleaseSigning {
    if ($script:RELEASE_REQUIRE_SIGNING -eq 1) {
        if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
            Stop-Release "TAURI_SIGNING_PRIVATE_KEY is not set (--require-signing)"
        }
        return
    }

    if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
        Write-ReleaseWarn "Unsigned build (no TAURI_SIGNING_PRIVATE_KEY; no Apple/Windows code signing required)"
    }
}

function Write-ReleaseGitDirtyWarn {
    & git diff --quiet
    $dirtyWorktree = $LASTEXITCODE -ne 0
    & git diff --cached --quiet
    $dirtyIndex = $LASTEXITCODE -ne 0
    if ($dirtyWorktree -or $dirtyIndex) {
        Write-ReleaseWarn "Working tree has uncommitted changes"
    }
}

function Assert-ReleaseCommand {
    param([Parameter(Mandatory = $true)][string]$Command)
    if (-not (Get-Command $Command -ErrorAction SilentlyContinue)) {
        Stop-Release "Required command not found: $Command"
    }
}

function Test-ReleaseNativeSuccess {
    param(
        [Parameter(Mandatory = $true)][string]$File,
        [string[]]$Arguments = @()
    )

    $old = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & $File @Arguments *> $null
        return $LASTEXITCODE -eq 0
    } finally {
        $ErrorActionPreference = $old
    }
}

function Test-ReleaseRemoteTagExists {
    param([Parameter(Mandatory = $true)][string]$Tag)
    $remote = if ($script:RELEASE_GIT_REMOTE) { $script:RELEASE_GIT_REMOTE } else { "origin" }
    return Test-ReleaseNativeSuccess "git" @("ls-remote", "--exit-code", "--tags", $remote, "refs/tags/$Tag")
}

function Test-ReleaseLocalTagExists {
    param([Parameter(Mandatory = $true)][string]$Tag)
    return Test-ReleaseNativeSuccess "git" @("rev-parse", "refs/tags/$Tag")
}
