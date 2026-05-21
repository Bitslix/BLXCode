# GitHub release upload - dot-source only.

function Test-ReleaseGhReleaseExists {
    param([Parameter(Mandatory = $true)][string]$Tag)
    if (-not (Get-Command "gh" -ErrorAction SilentlyContinue)) {
        return $false
    }
    return Test-ReleaseNativeSuccess "gh" @("release", "view", $Tag, "-R", $script:RELEASE_GH_REPO, "--json", "tagName")
}

function Get-ReleaseGhAssetNames {
    param([Parameter(Mandatory = $true)][string]$Tag)
    if (-not (Get-Command "gh" -ErrorAction SilentlyContinue)) {
        return @()
    }

    $old = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $assets = & gh release view $Tag -R $script:RELEASE_GH_REPO --json assets -q ".assets[].name" 2>$null
        if ($LASTEXITCODE -ne 0 -or -not $assets) {
            return @()
        }
        return @($assets)
    } finally {
        $ErrorActionPreference = $old
    }
}

function New-ReleaseGhRelease {
    param([Parameter(Mandatory = $true)][string]$Tag)

    if ($script:RELEASE_DRY_RUN -eq 1) {
        Write-ReleaseInfo "Would: gh release create $Tag -R $script:RELEASE_GH_REPO --draft --title BLXCode $Tag"
        return
    }

    & gh release create $Tag -R $script:RELEASE_GH_REPO --draft --title "BLXCode $Tag" --notes "See CHANGELOG.md in the repository."
    if ($LASTEXITCODE -ne 0) {
        Stop-Release "gh release create failed"
    }
}

function Get-ReleaseUploadFiles {
    param(
        [Parameter(Mandatory = $true)][string]$Tag,
        [Parameter(Mandatory = $true)][string[]]$Inputs
    )

    if ($script:RELEASE_CLOBBER -eq 1) {
        return $Inputs
    }

    $existing = @(Get-ReleaseGhAssetNames $Tag)
    $out = New-Object System.Collections.Generic.List[string]

    foreach ($file in $Inputs) {
        $base = Split-Path -Leaf $file
        if ($existing -contains $base) {
            Write-ReleaseWarn "Skipping existing asset: $base (use --clobber to replace)"
        } else {
            [void]$out.Add($file)
        }
    }

    return @($out)
}

function Invoke-ReleaseUploadArtifacts {
    param([Parameter(Mandatory = $true)][string]$Version)

    $tag = "v$Version"
    Assert-ReleaseCommand "gh"

    $artifacts = @(Get-ReleaseArtifactPaths $Version)
    if ($artifacts.Count -eq 0) {
        Stop-Release "No bundle artifacts under target/**/release/bundle/. Run --build first."
    }

    $releaseExists = $false
    if (Test-ReleaseGhReleaseExists $tag) {
        $releaseExists = $true
        Write-ReleaseInfo "Using existing GitHub release $tag"
    } elseif ((Test-ReleaseRemoteTagExists $tag) -or (Test-ReleaseLocalTagExists $tag)) {
        Write-ReleaseInfo "Tag $tag exists; creating draft release"
        New-ReleaseGhRelease $tag
        $releaseExists = $true
    } else {
        Write-ReleaseInfo "Creating new draft release $tag"
        New-ReleaseGhRelease $tag
        $releaseExists = $true
    }

    if ($releaseExists -and $script:RELEASE_CLOBBER -ne 1) {
        $toUpload = @(Get-ReleaseUploadFiles $tag $artifacts)
    } else {
        $toUpload = $artifacts
    }

    if ($toUpload.Count -eq 0) {
        Write-ReleaseWarn "Nothing to upload (all assets already on release?)"
        return
    }

    if ($script:RELEASE_DRY_RUN -eq 1) {
        Write-ReleaseInfo "Would upload to ${tag}:"
        foreach ($file in $toUpload) {
            Write-ReleaseInfo "  $file"
        }
        return
    }

    $ghArgs = @("release", "upload", $tag, "-R", $script:RELEASE_GH_REPO)
    if ($script:RELEASE_CLOBBER -eq 1) {
        $ghArgs += "--clobber"
    }
    $ghArgs += $toUpload

    & gh @ghArgs
    if ($LASTEXITCODE -ne 0) {
        Stop-Release "gh release upload failed"
    }
    Write-ReleaseInfo "Uploaded $($toUpload.Count) file(s) to $tag on $script:RELEASE_GH_REPO"
}

