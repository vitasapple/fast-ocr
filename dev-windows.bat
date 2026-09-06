@echo off
chcp 65001 >nul
echo ===================================================
echo   PaddleOCR 桌面版 - 本地开发预览
echo ===================================================
echo.
call npm install
call npm run tauri dev
pause
