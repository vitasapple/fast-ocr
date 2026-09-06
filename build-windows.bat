@echo off
chcp 65001 >nul
echo ===================================================
echo   大梨OCR 桌面版 - Windows 一键打包脚本
echo ===================================================
echo.

where node >nul 2>nul
if %errorlevel% neq 0 (
    echo [错误] 未检测到 Node.js，请先安装 Node.js: https://nodejs.org/
    pause
    exit /b 1
)

where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [错误] 未检测到 Rust (cargo)，请先安装 Rust: https://rustup.rs/
    echo 提示：安装时请选择默认的 MSVC (C++ Build Tools)。
    pause
    exit /b 1
)

echo [1/2] 正在安装 Node 依赖...
call npm install
if %errorlevel% neq 0 (
    echo [错误] npm install 失败。
    pause
    exit /b 1
)

echo.
echo [2/2] 正在执行 Tauri 打包 (生成 .exe / .msi 安装包)...
call npm run tauri build
if %errorlevel% neq 0 (
    echo [错误] 打包失败，请检查上方编译报错。
    pause
    exit /b 1
)

echo.
echo ===================================================
echo   打包成功！
echo   安装包生成在: src-tauri\target\release\bundle\nsis\
echo ===================================================
pause
