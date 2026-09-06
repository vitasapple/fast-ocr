# 大梨OCR 离线桌面版客户端 (PC / Windows / macOS)

基于 [Tauri v2](https://v2.tauri.app/) 构建的 **大梨OCR** 离线桌面版客户端。

---

## 💡 核心设计与无需 Python 说明

### 为什么这个软件完全不需要 Python？
- **全部计算在本地完成**：大梨OCR 的文本检测、文字识别以及 PDF 解析，100% 基于本地原生核心与 WebAssembly 引擎在 WebView 中离线运行，没有任何后端或云端计算。
- **体积小、无依赖**：**用户电脑上完全无需安装 Python，安装包内也无需内置任何 Python 运行库**，生成的安装包体积紧凑、启动秒开、开箱即用。
- **隐私安全**：离线运作，数据绝不经过任何第三方服务器，保护您的隐私与商业数据安全。

---

## 📦 离线识别引擎与模式说明

软件内置轻量化高精度离线 AI 识别引擎，提供三种识别模式以适应不同场景需求：
- **高精度模式（推荐）**：精准识别复杂排版、密集文字与多字体场景，准确率最高。
- **快速模式**：大幅提升处理速度，兼顾速度与准确度，适合日常大部分文档识别。
- **极速模式**：极致轻巧与极速响应，适合简单文本或低配置设备快速批处理。

所有引擎与离线资源均已随客户端打包内置，断网环境下亦可独立流畅运行。

---

## 🚀 打包生成 Windows 软件 (.exe / .msi)

你有两种方式可以打包生成 Windows PC 软件：

### 方式一：GitHub Actions 云端全自动打包（推荐，无需本地 Windows 环境）

如果你当前是在 Mac 或 Linux 电脑上开发，不想在本地配置复杂的 Windows 交叉编译环境，可以使用项目中已配置好的 GitHub Actions：

1. 将当前项目推送到你的 GitHub 仓库：
   ```bash
   git add .
   git commit -m "feat: update dali ocr desktop"
   git push
   ```
2. 进入 GitHub 仓库页面，点击顶部的 **Actions** 标签。
3. 会看到自动触发的 **Build Desktop App** 工作流（也可以手动点击 **Run workflow** 运行）。
4. 编译完成后，在工作流产物（Artifacts）中即可直接下载 Windows 安装包！

---

### 方式二：在 Windows 电脑上本地直接打包

如果你有一台 Windows 电脑（或 Windows 虚拟机），可以直接在本地打包：

#### 1. 前置环境准备（仅打包电脑需要，使用软件的终端用户不需要）
- **Node.js**：[Node.js 官方下载安装](https://nodejs.org/)（推荐 LTS 版本）
- **Rust**：访问 [rustup.rs](https://rustup.rs/) 下载安装 `rustup-init.exe`，安装时选择默认的 MSVC 工具链（Visual Studio C++ Build Tools）。
- **WebView2**：Windows 10/11 系统通常已内置；若旧系统缺失，运行软件时会自动提示安装。

#### 2. 安装项目依赖并打包
在项目根目录下双击运行 `build-windows.bat`，或在命令行执行：

```bash
# 1. 安装项目依赖
npm install

# 2. 本地调试运行（开发预览）
npm run dev

# 3. 正式打包为 Windows 安装包
npm run tauri build -- --bundles nsis
```

打包完成后，安装包将生成在：
- NSIS 安装程序（推荐）：`src-tauri/target/release/bundle/nsis/`

---

## 🍏 打包生成 macOS 软件 (.dmg / .app)

本项目全面支持 macOS 的**独立分包打包**，可分别打出针对 **Apple Silicon (M 系列芯片)** 与 **Intel 芯片** 的独立安装包，也支持打出通用的 Universal 安装包。

### 1. 前置环境准备（仅打包电脑需要）
- **Node.js**：[Node.js 官方下载](https://nodejs.org/)（推荐 LTS 版本，如 v18 或 v20+）
- **Rust**：在 macOS 终端执行官方命令安装：
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source $HOME/.cargo/env
  ```
- **安装交叉编译目标**（`build-mac.sh` 会自动检测并安装，亦可手动预装）：
  ```bash
  # 安装 Apple Silicon (M 系列) 编译目标
  rustup target add aarch64-apple-darwin

  # 安装 Intel (x86_64) 编译目标
  rustup target add x86_64-apple-darwin
  ```

---

### 2. 方式一：使用一键打包脚本（推荐）

在项目根目录下运行已配置好的 `build-mac.sh`：

```bash
# 赋予执行权限（首次使用）
chmod +x build-mac.sh

# 运行交互式打包菜单
./build-mac.sh
```

运行后可自由选择：
- **选项 1**：Apple Silicon M 系列独立包 (`aarch64-apple-darwin`)
- **选项 2**：Intel x86_64 独立包 (`x86_64-apple-darwin`)
- **选项 3**：分别打出两个独立包（同时生成 M 系列与 Intel 两个安装包）
- **选项 4**：Universal 通用二进制包（单个安装包自适应全部 Mac）

亦可使用命令行参数免交互直接打包：
```bash
./build-mac.sh arm64    # 打包 Apple Silicon 独立包
./build-mac.sh x64      # 打包 Intel 独立包
./build-mac.sh both     # 同时分别打两个独立包
./build-mac.sh universal# 打包通用包
```

---

### 3. 方式二：使用 npm 快捷指令打包

```bash
# 1. 安装依赖
npm install

# 2. 本地调试运行（开发预览）
npm run dev
# 或执行 ./dev-mac.sh

# 3. 分别打包独立安装包
npm run build:mac:arm64     # 仅打 Apple Silicon (M 系列) 独立包
npm run build:mac:x64       # 仅打 Intel (x86_64) 独立包
npm run build:mac:all       # 依次分别打出两个独立包

# 4. 打包 Universal 通用二进制包
npm run build:mac:universal
```

---

### 4. 安装包产物位置

打包完成后，DMG 镜像与 APP 程序将生成在：
- **Apple Silicon (M 系列)**：
  - DMG 镜像：`src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/`
  - APP 程序：`src-tauri/target/aarch64-apple-darwin/release/bundle/macos/大梨OCR.app`
- **Intel (x86_64)**：
  - DMG 镜像：`src-tauri/target/x86_64-apple-darwin/release/bundle/dmg/`
  - APP 程序：`src-tauri/target/x86_64-apple-darwin/release/bundle/macos/大梨OCR.app`
- **Universal 通用**：
  - DMG 镜像：`src-tauri/target/universal-apple-darwin/release/bundle/dmg/`

---

### 5. 💡 macOS 首次打开提示「已损坏」或「无法打开」的处理

由于本地构建的软件未经 Apple 商业证书公证，macOS Gatekeeper 安全机制可能会拦截：
1. **简易方式**：在 Finder（访达）中找到 `大梨OCR.app`，按住键盘 **Control 键** 并右键点击它，选择「**打开**」，在弹窗中点击「打开」即可。
2. **终端命令行方式**（彻底解除限制）：
   ```bash
   sudo xattr -rd com.apple.quarantine /Applications/大梨OCR.app
   ```

---

## 📂 项目工程结构

```
paddle-desktop/
├── package.json                  # Node 项目配置与 Tauri CLI 脚本
├── README.md                     # 本说明文档
├── build-mac.sh                  # macOS 一键打包独立包脚本 (Apple Silicon / Intel)
├── dev-mac.sh                    # macOS 本地调试开发脚本
├── build-windows.bat             # Windows 一键打包脚本 (.exe)
├── dev-windows.bat               # Windows 本地调试开发脚本
├── .gitignore                    # Git 忽略配置
├── .github/
│   └── workflows/
│       └── build.yml             # GitHub Actions 云端全自动打包工作流 (Win / macOS)
├── src/                          # 前端全部源码与离线资源
│   ├── index.html                # 单张图片 OCR 识别界面
│   ├── pdf.html                  # PDF 全篇幅 OCR 识别界面
│   ├── models/                   # 离线 ONNX 权重文件
│   ├── npm2/                     # 核心运行库与引擎
│   └── vendor/                   # PDF 核心库与 worker
└── src-tauri/                    # Tauri 原生 Rust 工程
    ├── Cargo.toml                # Rust 依赖配置
    ├── tauri.conf.json           # 桌面客户端窗口、图标、打包参数配置
    ├── build.rs                  # 编译脚本
    ├── icons/                    # 各尺寸应用图标（.ico / .png / .icns）
    └── src/
        └── main.rs               # 原生核心入口：启动安全嵌入式服务并打开主窗口
```

---

## 🖥️ 功能特性

- **单图文字识别**：支持选择图片、拖拽图片、剪贴板直接粘贴（Ctrl/⌘+V），快速框选与一键复制。
- **PDF 全篇幅识别**：支持多页长篇 PDF 逐页解析渲染并流水线识别，支持一键复制全部文本或下载为 `.txt`。
- **三种识别模式**：高精度模式、快速模式、极速模式，满足不同文档与性能诉求。
- **页面无缝切换**：单图识别与 PDF 识别页面顶部均自带高可见度切换按钮，随时切换模式。
- **全本地离线运算**：数据绝不经过任何第三方服务器，隐私安全无忧。
