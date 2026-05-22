@echo off
setlocal EnableExtensions
rem WiX light.exe (Tauri MSI) requires the VBSCRIPT optional Windows feature.
rem https://v2.tauri.app/distribute/windows-installer/

set "BUNDLE_ARGS=--verbose --bundles msi,nsis"

dism /online /Get-FeatureInfo /FeatureName:VBSCRIPT 2>nul | findstr /I /C:"State : Enabled" >nul
if not errorlevel 1 goto enabled

echo VBSCRIPT optional feature is disabled; attempting DISM enable (admin required)...
dism /online /Enable-Feature /FeatureName:VBSCRIPT /All /NoRestart
if errorlevel 1 goto nsis_only

dism /online /Get-FeatureInfo /FeatureName:VBSCRIPT 2>nul | findstr /I /C:"State : Enabled" >nul
if errorlevel 1 goto nsis_only

:enabled
echo VBSCRIPT is enabled; building MSI + NSIS bundles.
goto write_env

:nsis_only
echo WARNING: VBSCRIPT unavailable — skipping MSI, building NSIS setup only.
echo Enable manually: Settings ^> Apps ^> Optional features ^> More Windows features ^> VBSCRIPT
set "BUNDLE_ARGS=--verbose --bundles nsis"

:write_env
echo TAURI_BUILD_ARGS=%BUNDLE_ARGS%>>%GITHUB_ENV%
exit /b 0
