# Windows-focused launcher for the PowerShell release pipeline.
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $ScriptDir "release.ps1") @args --platform windows
exit $LASTEXITCODE

