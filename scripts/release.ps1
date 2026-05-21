# BLXCode release: optional semver bump, signed tauri build, GitHub release upload.
$ErrorActionPreference = "Stop"

$script:RELEASE_EXIT_USER = 1
$script:RELEASE_EXIT_BUILD = 2

$script:RELEASE_DRY_RUN = 0
$script:RELEASE_REQUIRE_SIGNING = 0
$script:RELEASE_NO_CHANGELOG = 0
$script:RELEASE_CLOBBER = 0
$script:RELEASE_DO_BUILD = 0
$script:RELEASE_DO_UPLOAD = 0
$script:RELEASE_UPLOAD_ONLY = 0
$script:RELEASE_DO_TAG = 0
$script:RELEASE_DO_PUSH = 0
$script:RELEASE_DO_COMMIT = 0
$script:RELEASE_BUMP = ""
$script:RELEASE_BUNDLES = ""
$script:RELEASE_PLATFORM_OVERRIDE = ""
$script:RELEASE_LINUX_ARCH = if ($env:RELEASE_LINUX_ARCH) { $env:RELEASE_LINUX_ARCH } else { "all" }
$script:RELEASE_VERSION = ""
$script:RELEASE_NO_BUILD = 0

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ReleaseScriptDir = Join-Path $ScriptDir "release"

. (Join-Path $ReleaseScriptDir "common.ps1")
. (Join-Path $ReleaseScriptDir "changelog.ps1")
. (Join-Path $ReleaseScriptDir "version.ps1")
. (Join-Path $ReleaseScriptDir "linux_targets.ps1")
. (Join-Path $ReleaseScriptDir "build.ps1")
. (Join-Path $ReleaseScriptDir "upload.ps1")

function Show-ReleaseUsage {
    @"
Usage: powershell -ExecutionPolicy Bypass -File scripts/release.ps1 [options]

Build signed Tauri bundles for the current host.
Optionally bump version, rewrite CHANGELOG, tag, push, and upload to GitHub.

Options:
  --bump patch|minor|major   Bump version in Cargo.toml + tauri.conf.json + CHANGELOG
  --no-changelog             Skip CHANGELOG rewrite on bump
  --build                    Run cargo tauri build (default when not --upload-only / --no-build)
  --no-build                 Skip build
  --bundles LIST             Pass to cargo tauri build --bundles (e.g. msi)
  --tag                      Create annotated git tag v{version}
  --push                     git push + push tag (requires --tag)
  --commit                   Commit version + CHANGELOG files
  --upload                   Upload bundle artifacts to GitHub release
  --upload-only              Only upload (no bump/build/tag)
  --clobber                  Replace existing release assets with same name
  --require-signing          Require TAURI_SIGNING_PRIVATE_KEY
  --allow-unsigned           Alias for default unsigned build (no-op)
  --platform linux|macos|windows
  --linux-arch native|amd64|arm64|all
                             Linux only: deb/rpm/AppImage per arch (default: all)
  --dry-run                  Print planned actions only
  -h, --help                 Show this help

Examples:
  scripts\release.cmd
  powershell -ExecutionPolicy Bypass -File scripts\release.ps1 --platform windows
  powershell -ExecutionPolicy Bypass -File scripts\release.ps1 --bump patch --build --upload
"@
}

$i = 0
while ($i -lt $args.Count) {
    switch ($args[$i]) {
        "--bump" {
            if ($i + 1 -ge $args.Count) { throw "--bump requires patch|minor|major" }
            $script:RELEASE_BUMP = $args[$i + 1]
            $i += 2
            continue
        }
        "--no-changelog" { $script:RELEASE_NO_CHANGELOG = 1; $i++; continue }
        "--build" { $script:RELEASE_DO_BUILD = 1; $i++; continue }
        "--no-build" { $script:RELEASE_DO_BUILD = 0; $script:RELEASE_NO_BUILD = 1; $i++; continue }
        "--bundles" {
            if ($i + 1 -ge $args.Count) { throw "--bundles requires a list" }
            $script:RELEASE_BUNDLES = $args[$i + 1]
            $i += 2
            continue
        }
        "--tag" { $script:RELEASE_DO_TAG = 1; $i++; continue }
        "--push" { $script:RELEASE_DO_PUSH = 1; $i++; continue }
        "--commit" { $script:RELEASE_DO_COMMIT = 1; $i++; continue }
        "--upload" { $script:RELEASE_DO_UPLOAD = 1; $i++; continue }
        "--upload-only" { $script:RELEASE_UPLOAD_ONLY = 1; $script:RELEASE_DO_UPLOAD = 1; $i++; continue }
        "--clobber" { $script:RELEASE_CLOBBER = 1; $i++; continue }
        "--require-signing" { $script:RELEASE_REQUIRE_SIGNING = 1; $i++; continue }
        "--allow-unsigned" { $i++; continue }
        "--platform" {
            if ($i + 1 -ge $args.Count) { throw "--platform requires linux|macos|windows" }
            $script:RELEASE_PLATFORM_OVERRIDE = $args[$i + 1]
            $i += 2
            continue
        }
        "--linux-arch" {
            if ($i + 1 -ge $args.Count) { throw "--linux-arch requires native|amd64|arm64|all" }
            $script:RELEASE_LINUX_ARCH = $args[$i + 1]
            $i += 2
            continue
        }
        "--dry-run" { $script:RELEASE_DRY_RUN = 1; $i++; continue }
        "-h" { Show-ReleaseUsage; exit 0 }
        "--help" { Show-ReleaseUsage; exit 0 }
        default { throw "Unknown option: $($args[$i]) (use --help)" }
    }
}

Initialize-ReleaseCommon $ReleaseScriptDir
Write-ReleaseInfo "Git remote: $script:RELEASE_GIT_REMOTE | GitHub: $script:RELEASE_GH_REPO"

if ($script:RELEASE_UPLOAD_ONLY -eq 1) {
    $script:RELEASE_DO_BUILD = 0
} elseif ($script:RELEASE_NO_BUILD -ne 1 -and $script:RELEASE_DO_BUILD -eq 0 -and $script:RELEASE_DO_TAG -eq 0 -and -not $script:RELEASE_BUMP) {
    $script:RELEASE_DO_BUILD = 1
} elseif ($script:RELEASE_BUMP -and $script:RELEASE_NO_BUILD -ne 1 -and $script:RELEASE_DO_BUILD -eq 0 -and $script:RELEASE_UPLOAD_ONLY -ne 1) {
    $script:RELEASE_DO_BUILD = 1
}

if ($script:RELEASE_DO_PUSH -eq 1 -and $script:RELEASE_DO_TAG -ne 1) {
    Stop-Release "--push requires --tag"
}

$script:RELEASE_VERSION = Read-ReleaseVersion
$platform = Get-ReleasePlatform
Write-ReleaseInfo "Project version: $script:RELEASE_VERSION (platform: $platform)"

Import-ReleaseEnv

$tagCurrent = "v$script:RELEASE_VERSION"
if (Test-ReleaseGhReleaseExists $tagCurrent) {
    Write-ReleaseInfo "GitHub release $tagCurrent already exists"
} elseif (Test-ReleaseRemoteTagExists $tagCurrent) {
    Write-ReleaseInfo "Remote tag $tagCurrent already exists"
}

if ($script:RELEASE_UPLOAD_ONLY -eq 1) {
    $script:RELEASE_BUMP = ""
    $script:RELEASE_DO_TAG = 0
    $script:RELEASE_DO_BUILD = 0
}

if ($script:RELEASE_BUMP) {
    Invoke-ReleaseBumpVersion $script:RELEASE_BUMP
    $tagCurrent = "v$script:RELEASE_VERSION"
    if ((Test-ReleaseGhReleaseExists $tagCurrent) -or (Test-ReleaseRemoteTagExists $tagCurrent)) {
        Stop-Release "Release $tagCurrent already exists on GitHub after bump"
    }
}

if ($script:RELEASE_DO_BUILD -eq 1) {
    Invoke-ReleasePrepareDeps
    try {
        Invoke-ReleaseBuild
    } catch {
        Write-ReleaseError $_.Exception.Message
        exit $script:RELEASE_EXIT_BUILD
    }
    Write-ReleaseInfo "Build artifacts (v$script:RELEASE_VERSION):"
    Show-ReleaseArtifacts $script:RELEASE_VERSION
}

if ($script:RELEASE_DO_COMMIT -eq 1) {
    if ($script:RELEASE_DRY_RUN -eq 1) {
        Write-ReleaseInfo "Would: git commit version + CHANGELOG"
    } else {
        & git add CHANGELOG.md Cargo.toml src-tauri/Cargo.toml src-tauri/tauri.conf.json
        $releaseStatus = (& git status --porcelain scripts/release.sh scripts/release/ .gitignore docs/user/building.md scripts/release.ps1 scripts/release.cmd 2>$null)
        if ($releaseStatus) {
            & git add scripts/release.sh scripts/release/ .gitignore docs/user/building.md scripts/release.ps1 scripts/release.cmd
        }
        & git commit -m "chore: release v$script:RELEASE_VERSION"
        if ($LASTEXITCODE -ne 0) { Stop-Release "git commit failed" }
        Write-ReleaseInfo "Committed release files"
    }
}

if ($script:RELEASE_DO_TAG -eq 1) {
    Write-ReleaseGitDirtyWarn
    if ((Test-ReleaseRemoteTagExists $tagCurrent) -or (Test-ReleaseLocalTagExists $tagCurrent)) {
        Write-ReleaseWarn "Tag $tagCurrent already exists; skipping git tag (use --upload to add assets)"
    } elseif ($script:RELEASE_DRY_RUN -eq 1) {
        Write-ReleaseInfo "Would: git tag -a $tagCurrent -m `"BLXCode $script:RELEASE_VERSION`""
    } else {
        & git tag -a $tagCurrent -m "BLXCode $script:RELEASE_VERSION"
        if ($LASTEXITCODE -ne 0) { Stop-Release "git tag failed" }
        Write-ReleaseInfo "Created tag $tagCurrent"
    }
}

if ($script:RELEASE_DO_PUSH -eq 1) {
    if ($script:RELEASE_DRY_RUN -eq 1) {
        Write-ReleaseInfo "Would: git push $script:RELEASE_GIT_REMOTE && git push $script:RELEASE_GIT_REMOTE $tagCurrent"
    } else {
        & git push $script:RELEASE_GIT_REMOTE
        if ($LASTEXITCODE -ne 0) { Stop-Release "git push failed" }
        if (Test-ReleaseLocalTagExists $tagCurrent) {
            if (-not (Test-ReleaseRemoteTagExists $tagCurrent)) {
                & git push $script:RELEASE_GIT_REMOTE $tagCurrent
                if ($LASTEXITCODE -ne 0) { Stop-Release "git tag push failed" }
                Write-ReleaseInfo "Pushed tag $tagCurrent to $script:RELEASE_GIT_REMOTE"
            } else {
                Write-ReleaseInfo "Tag $tagCurrent already on $script:RELEASE_GIT_REMOTE; skipping tag push"
            }
        }
    }
}

if ($script:RELEASE_DO_UPLOAD -eq 1) {
    Invoke-ReleaseUploadArtifacts $script:RELEASE_VERSION
}

Write-ReleaseInfo "Done."

