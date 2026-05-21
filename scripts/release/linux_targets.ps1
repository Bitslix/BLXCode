# Linux multi-arch release builds - dot-source only.

function Get-ReleaseLinuxCpu {
    $machine = (& uname -m)
    switch ($machine) {
        { $_ -in @("x86_64", "amd64") } { return "amd64" }
        { $_ -in @("aarch64", "arm64") } { return "arm64" }
        default { return $machine }
    }
}

function Test-ReleaseLinuxCrossLinkerReady {
    param([Parameter(Mandatory = $true)][string]$Triple)
    switch ($Triple) {
        "aarch64-unknown-linux-gnu" { return [bool](Get-Command "aarch64-linux-gnu-gcc" -ErrorAction SilentlyContinue) }
        "x86_64-unknown-linux-gnu" { return [bool](Get-Command "x86_64-linux-gnu-gcc" -ErrorAction SilentlyContinue) }
        default { return $false }
    }
}

function Test-ReleaseLinuxCrossPkgConfigReady {
    param(
        [Parameter(Mandatory = $true)][string]$Triple,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $required = @("glib-2.0", "gobject-2.0", "gio-2.0", "gtk+-3.0", "webkit2gtk-4.1")
    $wrapper = ""
    $libdir = ""

    switch ($Triple) {
        "aarch64-unknown-linux-gnu" {
            $wrapper = "aarch64-linux-gnu-pkg-config"
            $libdir = "/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
        }
        "x86_64-unknown-linux-gnu" {
            $wrapper = "x86_64-linux-gnu-pkg-config"
            $libdir = "/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig"
        }
        default { return $false }
    }

    if ((Get-Command $wrapper -ErrorAction SilentlyContinue) -and (Test-ReleaseNativeSuccess $wrapper (@("--exists") + $required))) {
        $env:PKG_CONFIG = $wrapper
        $env:PKG_CONFIG_ALLOW_CROSS = "1"
        return $true
    }

    $firstLibdir = ($libdir -split ":", 2)[0]
    if ((Test-Path $firstLibdir) -and (Get-Command "pkg-config" -ErrorAction SilentlyContinue)) {
        $oldAllow = $env:PKG_CONFIG_ALLOW_CROSS
        $oldLibdir = $env:PKG_CONFIG_LIBDIR
        $env:PKG_CONFIG_ALLOW_CROSS = "1"
        $env:PKG_CONFIG_LIBDIR = $libdir
        if (Test-ReleaseNativeSuccess "pkg-config" (@("--exists") + $required)) {
            return $true
        }
        $env:PKG_CONFIG_ALLOW_CROSS = $oldAllow
        $env:PKG_CONFIG_LIBDIR = $oldLibdir
    }

    Write-ReleaseWarn "Skipping $Label cross build: pkg-config sysroot for $Triple is not configured"
    Write-ReleaseWarn "Need target .pc files for: $($required -join ' ')"
    return $false
}

function Export-ReleaseLinuxAppImageEnv {
    $env:NO_STRIP = "1"
    $env:APPIMAGE_EXTRACT_AND_RUN = "1"
    Write-ReleaseInfo "Linux: NO_STRIP=1 APPIMAGE_EXTRACT_AND_RUN=1 (AppImage strip workaround)"
}

function Invoke-ReleaseLinuxTauriBuild {
    param([string[]]$ExtraArgs = @())

    $cargoArgs = @("tauri", "build") + $ExtraArgs
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
        if ($LASTEXITCODE -ne 0) {
            throw "cargo $($cargoArgs -join ' ') failed"
        }
    } finally {
        Pop-Location
    }
}

function Invoke-ReleaseLinuxBuildNative {
    $cpu = Get-ReleaseLinuxCpu
    Write-ReleaseInfo "Linux native build ($cpu): deb, rpm, AppImage"
    Export-ReleaseLinuxAppImageEnv
    Invoke-ReleaseLinuxTauriBuild
}

function Invoke-ReleaseLinuxBuildCross {
    param(
        [Parameter(Mandatory = $true)][string]$Triple,
        [Parameter(Mandatory = $true)][string]$Label
    )

    if (-not (Test-ReleaseLinuxCrossLinkerReady $Triple)) {
        Write-ReleaseWarn "Skipping $Label cross build: linker for $Triple not found (install cross gcc)"
        return
    }
    if (-not (Test-ReleaseLinuxCrossPkgConfigReady $Triple $Label)) {
        return
    }

    Write-ReleaseInfo "Linux cross build ($Label / $Triple): deb, rpm"
    Export-ReleaseLinuxAppImageEnv
    Invoke-ReleaseLinuxTauriBuild @("--target", $Triple, "--bundles", "deb,rpm")
}

function Add-ReleaseLinuxTargets {
    if ($script:RELEASE_DRY_RUN -eq 1) {
        Write-ReleaseInfo "Would: rustup target add aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu"
        return
    }

    & rustup target add aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu *> $null
}

function Invoke-ReleaseLinuxBuildAll {
    $mode = if ($script:RELEASE_LINUX_ARCH) { $script:RELEASE_LINUX_ARCH } else { "all" }
    $cpu = Get-ReleaseLinuxCpu

    Add-ReleaseLinuxTargets

    switch ($mode) {
        { $_ -in @("native") } { Invoke-ReleaseLinuxBuildNative }
        { $_ -in @("amd64", "x86_64", "x64") } {
            if ($cpu -eq "amd64") {
                Invoke-ReleaseLinuxBuildNative
            } else {
                Invoke-ReleaseLinuxBuildCross "x86_64-unknown-linux-gnu" "amd64"
                Write-ReleaseWarn "AppImage amd64: run on an x86_64 Linux host (not cross-built here)"
            }
        }
        { $_ -in @("arm64", "aarch64", "arm") } {
            if ($cpu -eq "arm64") {
                Invoke-ReleaseLinuxBuildNative
            } else {
                Invoke-ReleaseLinuxBuildCross "aarch64-unknown-linux-gnu" "arm64"
                Write-ReleaseWarn "AppImage arm64: run on an aarch64 Linux host or use CI arm runner"
            }
        }
        default {
            Invoke-ReleaseLinuxBuildNative
            if ($cpu -eq "amd64") {
                Invoke-ReleaseLinuxBuildCross "aarch64-unknown-linux-gnu" "arm64"
            } else {
                Invoke-ReleaseLinuxBuildCross "x86_64-unknown-linux-gnu" "amd64"
            }
        }
    }
}

