@echo off
setlocal enabledelayedexpansion

echo Agent1 - Starting
echo ==============
echo.

REM Check for Node.js
where node >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo ERROR: Node.js is not installed. Please install from https://nodejs.org
    exit /b 1
)

REM Check for Rust/Cargo
where cargo >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo ERROR: Rust is not installed. Please install from https://rustup.rs
    exit /b 1
)

echo [1/4] Checking WhatsApp sidecar...
cd /d "%~dp0whatsapp-sidecar"
if not exist "node_modules" (
    echo Installing WhatsApp sidecar dependencies...
    call npm install --yes
)
cd /d "%~dp0"

echo [2/4] Starting WhatsApp sidecar in background...
start /b "WhatsApp Sidecar" cmd /c "cd /d %~dp0whatsapp-sidecar && npm start"

echo Waiting for sidecar to start...
timeout /t 3 /nobreak >nul

echo [3/4] Starting API server in background...
start /b "Agent1 Server" cmd /c "cargo run --bin agent1 -- server"

echo Waiting for server to start...
timeout /t 3 /nobreak >nul

echo [4/4] Building desktop UI...
cd desktop
if not exist "node_modules" (
    echo Installing desktop dependencies...
    call npm install
)
call npm run build
cd ..

echo.
echo Agent1 is starting!
echo ==============
echo - API Server: http://127.0.0.1:17371
echo - WhatsApp Sidecar: http://127.0.0.1:17372
echo.
echo Starting Tauri dev mode...
echo.
cd desktop
call npm run tauri:dev
cd ..