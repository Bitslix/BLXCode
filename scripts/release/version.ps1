# Semver bump and sync version files - dot-source only.

function Get-ReleaseBumpedVersion {
    param(
        [Parameter(Mandatory = $true)][string]$Current,
        [Parameter(Mandatory = $true)][string]$Part
    )

    $pieces = $Current -split "\."
    if ($pieces.Count -ne 3 -or ($pieces | Where-Object { $_ -notmatch "^\d+$" })) {
        Stop-Release "invalid semver: '$Current'"
    }

    $major = [int]$pieces[0]
    $minor = [int]$pieces[1]
    $patch = [int]$pieces[2]

    # Accept "patch", "minor", "major" (+1) or "patch+N", "minor+N", "major+N" (+N).
    $mo = [regex]::Match($Part, '^(patch|minor|major)(?:\+(\d+))?$')
    if (-not $mo.Success) {
        Stop-Release "unknown bump spec: '$Part' (use patch|minor|major[+N])"
    }
    $kind = $mo.Groups[1].Value
    $step = 1
    if ($mo.Groups[2].Success) {
        $step = [int]$mo.Groups[2].Value
    }
    if ($step -lt 1) {
        Stop-Release "bump step must be >= 1, got $step"
    }

    switch ($kind) {
        "patch" { $patch += $step }
        "minor" { $minor += $step; $patch = 0 }
        "major" { $major += $step; $minor = 0; $patch = 0 }
    }

    return "$major.$minor.$patch"
}

function Invoke-ReleaseBumpVersion {
    param([Parameter(Mandatory = $true)][string]$Part)

    $current = Read-ReleaseVersion
    $new = Get-ReleaseBumpedVersion $current $Part

    if ($script:RELEASE_DRY_RUN -eq 1) {
        Write-ReleaseInfo "Would bump $current -> $new ($Part)"
        $script:RELEASE_VERSION = $new
        if ($script:RELEASE_NO_CHANGELOG -ne 1) {
            Invoke-ReleaseChangelogFinalize $new
        }
        return
    }

    $conf = Get-ReleaseTauriConfPath
    $jsonText = [System.IO.File]::ReadAllText($conf, [System.Text.UTF8Encoding]::new($false, $true))
    $json = $jsonText | ConvertFrom-Json
    $json.version = $new
    [System.IO.File]::WriteAllText(
        $conf,
        (($json | ConvertTo-Json -Depth 32) + [Environment]::NewLine),
        [System.Text.UTF8Encoding]::new($false)
    )

    foreach ($rel in @("Cargo.toml", "src-tauri\Cargo.toml")) {
        $path = Join-Path $script:RELEASE_ROOT $rel
        $text = [System.IO.File]::ReadAllText($path, [System.Text.UTF8Encoding]::new($false, $true))
        $regex = [regex]::new('(?m)^(version\s*=\s*")[^"]+(")')
        if ($regex.Matches($text).Count -lt 1) {
            Stop-Release "${path}: could not update version = ..."
        }
        $updated = $regex.Replace($text, "`${1}$new`$2", 1)
        [System.IO.File]::WriteAllText($path, $updated, [System.Text.UTF8Encoding]::new($false))
    }

    Write-ReleaseInfo "Bumped $current -> $new ($Part)"
    $script:RELEASE_VERSION = $new

    if ($script:RELEASE_NO_CHANGELOG -ne 1) {
        Invoke-ReleaseChangelogFinalize $new
    }
}

function Test-ReleaseBumpConflict {
    param([Parameter(Mandatory = $true)][string]$Version)
    $tag = "v$Version"
    if ((Test-ReleaseRemoteTagExists $tag) -or (Test-ReleaseGhReleaseExists $tag)) {
        Stop-Release "Release $tag already exists on GitHub; refusing --bump to $Version"
    }
}
