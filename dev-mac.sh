#!/usr/bin/env bash
set -e

export PATH="$HOME/.cargo/bin:/usr/local/bin:/opt/homebrew/bin:$PATH"

echo "==================================================="
echo "  大梨OCR - macOS 本地开发预览"
echo "==================================================="
echo ""

# 检查是否安装了 Rust / Cargo
if command -v cargo >/dev/null 2>&1; then
    echo "✓ 已检测到 Rust 环境 (cargo)"
    echo "请选择运行模式:"
    echo "  1) 原生桌面窗口 (Tauri Desktop App)"
    echo "  2) 本地浏览器极速预览 (无需编译，秒开)"
    read -r -p "请输入选项 [1-2] (默认为 1): " CHOICE
    CHOICE="${CHOICE:-1}"

    if [ "$CHOICE" = "1" ]; then
        if [ ! -d "node_modules" ] || [ ! -d "node_modules/@tauri-apps/cli" ]; then
            npm install
        fi
        exec npm run tauri dev
    fi
fi

# 浏览器极速预览模式
echo "⚡ 正在以本地极速预览模式启动..."
echo "✓ 自动开启 COOP/COEP 支持（多线程 WASM + WebGPU）"
echo "🌐 正在打开浏览器访问: http://127.0.0.1:8000"
echo ""

if ! command -v cargo >/dev/null 2>&1; then
    echo "💡 提示：当前 Mac 尚未安装 Rust (cargo)。"
    echo "   若后续需编译打包 macOS 独立安装包 (.dmg/.app)，可运行官方命令安装 Rust:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo ""
fi

(sleep 1 && open "http://127.0.0.1:8000") &
exec python3 server.py 8000
