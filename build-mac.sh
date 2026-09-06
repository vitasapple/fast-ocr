#!/usr/bin/env bash
set -e

# Ensure PATH includes Cargo and NVM / Homebrew common paths
export PATH="$HOME/.cargo/bin:/usr/local/bin:/opt/homebrew/bin:$PATH"

echo "==================================================="
echo "  大梨OCR 桌面版 - macOS 一键独立打包脚本"
echo "  (支持 Apple Silicon M 系列 与 Intel x86_64 独立包)"
echo "==================================================="
echo ""

# 1. 检查 Node.js
if ! command -v node >/dev/null 2>&1; then
    echo "❌ [错误] 未检测到 Node.js，请先安装 Node.js (推荐 v18 或 v20+): https://nodejs.org/"
    exit 1
fi

# 2. 检查 Rust & Cargo
if ! command -v cargo >/dev/null 2>&1; then
    echo "❌ [错误] 未检测到 Rust (cargo)。"
    echo "请在终端运行以下官方命令安装 Rust 工具链:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "安装完成后请重启终端或执行 source \$HOME/.cargo/env 后重试。"
    exit 1
fi

if ! command -v rustup >/dev/null 2>&1; then
    echo "❌ [错误] 未检测到 rustup，无法配置多架构目标。"
    exit 1
fi

# 3. 检查并安装项目 Node 依赖
if [ ! -d "node_modules" ] || [ ! -d "node_modules/@tauri-apps/cli" ]; then
    echo "📦 [1/3] 正在安装前端与 Tauri CLI 依赖..."
    npm install
else
    echo "✓ 前端依赖已就绪"
fi

echo ""

# 4. 选择构建架构
CHOICE="$1"
if [ -z "$CHOICE" ]; then
    echo "请选择要打包的 macOS 架构类型:"
    echo "  1) Apple Silicon M 系列独立包 (M1/M2/M3/M4 - aarch64) [推荐]"
    echo "  2) Intel x86_64 独立包 (Core i3/i5/i7/i9 / Xeon)"
    echo "  3) 分别打出两个独立包 (同时生成 M 系列与 Intel 两个安装包)"
    echo "  4) Universal 通用二进制包 (单个安装包自适应全部 Mac)"
    echo ""
    read -r -p "请输入选项 [1-4] (默认为 1): " INPUT_CHOICE
    CHOICE="${INPUT_CHOICE:-1}"
fi

case "$CHOICE" in
    1|m|m1|arm64|aarch64)
        BUILD_MODE="arm64"
        ;;
    2|intel|x64|x86_64)
        BUILD_MODE="x64"
        ;;
    3|both|all)
        BUILD_MODE="both"
        ;;
    4|universal)
        BUILD_MODE="universal"
        ;;
    *)
        echo "⚠️ 未知选项 '$CHOICE'，默认使用 1 (Apple Silicon M 系列)"
        BUILD_MODE="arm64"
        ;;
esac

# 辅助函数：确保 rustup 目标已安装
ensure_target() {
    local target="$1"
    if ! rustup target list --installed | grep -q "^${target}\$"; then
        echo "📥 正在自动安装 Rust 编译目标: ${target} ..."
        rustup target add "${target}"
    else
        echo "✓ Rust 编译目标已就绪: ${target}"
    fi
}

build_arm64() {
    echo ""
    echo "🚀 开始打包 Apple Silicon (M 系列 / arm64) 独立安装包..."
    ensure_target "aarch64-apple-darwin"
    npm run build:mac:arm64
    echo "✓ Apple Silicon 安装包生成完毕！"
}

build_x64() {
    echo ""
    echo "🚀 开始打包 Intel (x86_64) 独立安装包..."
    ensure_target "x86_64-apple-darwin"
    npm run build:mac:x64
    echo "✓ Intel 安装包生成完毕！"
}

build_universal() {
    echo ""
    echo "🚀 开始打包 Universal 通用二进制 (同时兼容 Intel 与 M 系列)..."
    ensure_target "aarch64-apple-darwin"
    ensure_target "x86_64-apple-darwin"
    npm run build:mac:universal
    echo "✓ Universal 安装包生成完毕！"
}

# 执行打包
case "$BUILD_MODE" in
    arm64)
        build_arm64
        ;;
    x64)
        build_x64
        ;;
    both)
        build_arm64
        build_x64
        ;;
    universal)
        build_universal
        ;;
esac

echo ""
echo "==================================================="
echo "🎉 打包流程已完成！"
echo "==================================================="
echo "产物路径如下:"

if [ "$BUILD_MODE" = "arm64" ] || [ "$BUILD_MODE" = "both" ]; then
    echo ""
    echo "🍏 Apple Silicon (M 系列) 安装包目录:"
    echo "   DMG 镜像: src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/"
    echo "   APP 程序: src-tauri/target/aarch64-apple-darwin/release/bundle/macos/大梨OCR.app"
fi

if [ "$BUILD_MODE" = "x64" ] || [ "$BUILD_MODE" = "both" ]; then
    echo ""
    echo "💻 Intel (x86_64) 安装包目录:"
    echo "   DMG 镜像: src-tauri/target/x86_64-apple-darwin/release/bundle/dmg/"
    echo "   APP 程序: src-tauri/target/x86_64-apple-darwin/release/bundle/macos/大梨OCR.app"
fi

if [ "$BUILD_MODE" = "universal" ]; then
    echo ""
    echo "🌐 Universal (通用架构) 安装包目录:"
    echo "   DMG 镜像: src-tauri/target/universal-apple-darwin/release/bundle/dmg/"
    echo "   APP 程序: src-tauri/target/universal-apple-darwin/release/bundle/macos/大梨OCR.app"
fi

echo ""
echo "💡 提示：对于未签名的 macOS 应用，若直接打开提示「已损坏」或「无法打开」："
echo "   可打开终端执行以下命令移除隔离属性:"
echo "   sudo xattr -rd com.apple.quarantine /Applications/大梨OCR.app"
echo "   或者：在 Finder 中按住 Control 键右键点击应用，选择「打开」即可。"
echo "==================================================="
