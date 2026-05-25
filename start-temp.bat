@echo off
setlocal

cd /d "%~dp0"

if not exist "node_modules" (
  echo [start-temp] node_modules not found, installing dependencies...
  call npm install
  if errorlevel 1 (
    echo [start-temp] npm install failed.
    pause
    exit /b 1
  )
)

echo [start-temp] starting tauri dev...
call npm run tauri dev

if errorlevel 1 (
  echo.
  echo [start-temp] startup failed.
  pause
  exit /b 1
)

endlocal
