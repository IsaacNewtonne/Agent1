@echo off
setlocal enabledelayedexpansion

echo Agent1 - Starting
echo ==============

echo [1/3] Starting API server in background...
start /b "Agent1 Server" cargo run -q --bin agent1 -- server

echo Waiting for server to start...
timeout /t 2 /nobreak >nul

echo [2/3] Building and opening desktop UI...
cd desktop
call npm install
call npm run tauri:build
cd ..

echo [3/3] Launching desktop...
for /r "desktop\src-tauri\target\release" %%f in (*.exe) do (
    start "" "%%f"
    goto :found
)

:found
if not defined found (
    echo ERROR: Could not find built exe
    exit /b 1
)

echo.
echo Agent1 is starting!
echo - API Server: http://127.0.0.1:17371
echo - Desktop: Should open automatically
echo.
echo Press any key to exit this window...
pause >nul