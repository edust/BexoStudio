@echo off
setlocal
pushd "%~dp0"
if errorlevel 1 exit /b 1

where node >nul 2>&1
if errorlevel 1 goto missing_tools
where npm >nul 2>&1
if errorlevel 1 goto missing_tools
where cargo >nul 2>&1
if errorlevel 1 goto missing_tools

node -e "if (process.platform !== 'win32' || process.arch !== 'x64') { console.error('Please use Windows x64 Node.js.'); process.exit(1); }"
if errorlevel 1 goto failed

if exist "node_modules\@tauri-apps\cli\tauri.js" goto build
call npm ci
if errorlevel 1 goto failed

:build
call npm run release:build
if errorlevel 1 goto failed
echo.
echo Build complete. Installer directory:
echo "%CD%\src-tauri\target\x86_64-pc-windows-msvc\release\bundle\nsis\"
popd
pause
exit /b 0

:missing_tools
echo Install Node.js 24/npm, Rust stable, MSVC Build Tools and WebView2 first.
goto failed

:failed
echo Build failed. See the errors above.
popd
pause
exit /b 1
