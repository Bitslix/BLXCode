# Pre-build deps and cargo tauri build - dot-source only.

function Invoke-ReleasePrepareDeps {
    Assert-ReleaseCommand "npm"
    Assert-ReleaseCommand "cargo"
    Assert-ReleaseCommand "rustup"

    if ($script:RELEASE_DRY_RUN -eq 1) {
        Write-ReleaseInfo "Would: rustup target add wasm32-unknown-unknown"
        Write-ReleaseInfo "Would: npm ci --prefix frontend-js && npm run build:graph3d"
        return
    }

    & rustup target add wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { Stop-Release "rustup target add wasm32-unknown-unknown failed" }

    $frontend = Join-Path $script:RELEASE_ROOT "frontend-js"
    if (Test-Path (Join-Path $frontend "package-lock.json")) {
        & npm ci --prefix $frontend
    } else {
        & npm install --prefix $frontend
    }
    if ($LASTEXITCODE -ne 0) { Stop-Release "npm install failed" }

    & npm --prefix $frontend run build:graph3d
    if ($LASTEXITCODE -ne 0) { Stop-Release "npm run build:graph3d failed" }
}

function Invoke-ReleaseMacOSBuild {
    if ($script:RELEASE_DRY_RUN -eq 1) {
        Write-ReleaseInfo "Would: rustup target add aarch64-apple-darwin x86_64-apple-darwin"
        Write-ReleaseInfo "Would: cargo tauri build --target universal-apple-darwin (Apple Silicon + Intel)"
        return
    }

    & rustup target add aarch64-apple-darwin x86_64-apple-darwin
    if ($LASTEXITCODE -ne 0) { Stop-Release "rustup target add macOS targets failed" }

    Write-ReleaseInfo "macOS: universal binary (aarch64 + x86_64) for Apple Silicon and Intel Macs"
    Test-ReleaseSigning
    Push-Location (Join-Path $script:RELEASE_ROOT "src-tauri")
    try {
        & cargo tauri build --target universal-apple-darwin
        if ($LASTEXITCODE -ne 0) { throw "cargo tauri build failed" }
    } finally {
        Pop-Location
    }
}

function Invoke-ReleaseWindowsBuild {
    $cargoArgs = @("tauri", "build")
    if ($script:RELEASE_BUNDLES) {
        $cargoArgs += @("--bundles", $script:RELEASE_BUNDLES)
    }

    if ($script:RELEASE_DRY_RUN -eq 1) {
        Write-ReleaseInfo "Would: cargo $($cargoArgs -join ' ')"
        return
    }

    Test-ReleaseSigning
    Push-Location (Join-Path $script:RELEASE_ROOT "src-tauri")
    try {
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) { throw "cargo $($cargoArgs -join ' ') failed" }
    } finally {
        Pop-Location
    }
}

function Invoke-ReleaseBuild {
    $platform = Get-ReleasePlatform
    switch ($platform) {
        "linux" { Invoke-ReleaseLinuxBuildAll }
        "macos" { Invoke-ReleaseMacOSBuild }
        "windows" { Invoke-ReleaseWindowsBuild }
        default { Stop-Release "Unknown platform: $platform" }
    }
}

function Get-ReleaseArtifactPaths {
    param([string]$Version = "")

    $target = Join-Path $script:RELEASE_ROOT "target"
    if (-not (Test-Path $target)) {
        return @()
    }

    $allowed = @(".deb", ".rpm", ".AppImage", ".appimage", ".dmg", ".msi", ".exe", ".sig")
    $files = Get-ChildItem -Path $target -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object {
            $_.FullName -match "[/\\]release[/\\]bundle[/\\]" -and
            (($allowed -contains $_.Extension) -or $_.Name.EndsWith(".app.tar.gz"))
        }

    if ($Version) {
        $files = $files | Where-Object { $_.Name -like "*$Version*" }
    }

    return @($files | ForEach-Object { $_.FullName })
}

function Show-ReleaseArtifacts {
    param([string]$Version = $script:RELEASE_VERSION)

    foreach ($path in Get-ReleaseArtifactPaths $Version) {
        $item = Get-Item $path
        $sizeMb = [math]::Round($item.Length / 1MB, 2)
        Write-ReleaseInfo "  $path (${sizeMb} MB)"
    }
}

