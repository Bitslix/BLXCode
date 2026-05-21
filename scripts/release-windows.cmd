@echo off
setlocal
set "SCRIPT_DIR=%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%release.ps1" %* --platform windows
exit /b %ERRORLEVEL%

