#!/usr/bin/env bash
set -e

export PATH="$HOME/.cargo/bin:/usr/local/bin:/opt/homebrew/bin:$PATH"

echo "==================================================="
echo "  大梨OCR 桌面版 - macOS 本地开发预览"
echo "==================================================="
echo ""

if [ ! -d "node_modules" ] || [ ! -d "node_modules/@tauri-apps/cli" ]; then
    npm install
fi

npm run tauri dev
