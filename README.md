# PaddleOCR.js 桌面版客户端 (PC / Windows / macOS)

基于 [Tauri v2](https://v2.tauri.app/) 构建的 PaddleOCR 离线桌面版 PC 客户端。

---

## 💡 核心设计与 Python 说明

### 为什么这个软件完全不需要 Python？
在最初的网页版本中，`server.py` 只是一个本地简易 HTTP 静态文件服务器，它的唯一作用是返回 `Cross-Origin-Opener-Policy: same-origin` 和 `Cross-Origin-Embedder-Policy: credentialless` 等响应头，让浏览器开启多线程 WebAssembly（SharedArrayBuffer）。

- **全部计算在本地完成**：PaddleOCR 的文本检测、文字识别以及 PDF.js 解析，100% 是基于前端 **WebAssembly（ONNX Runtime Web）** 与 JavaScript 在本地 WebView 引擎中运行的，没有任何 Python 端计算逻辑。
- **Tauri 原生替代**：本项目由 Tauri 原生 Rust 核心接管静态资源托管与安全标头注入，完全脱离了对 Python 的依赖。
- **体积小、无依赖**：**最终用户的 Windows 电脑上完全无需安装 Python，安装包内也无需内置任何 Python 运行库**，生成的 Windows 安装包体积紧凑、启动秒开、开箱即用。

---

## 📦 离线模型内置说明

当前项目已完整内置 6 款高精度与轻量化 ONNX 离线模型（位于 `src/models/`）：
- `PP-OCRv5_mobile_det` + `PP-OCRv5_mobile_rec`（推荐，中文高精度）
- `PP-OCRv6_small_det` + `PP-OCRv6_small_rec`（轻量快速）
- `PP-OCRv6_tiny_det` + `PP-OCRv6_tiny_rec`（极致轻巧）

所有模型及 ONNX Runtime WebAssembly 运行时全部封装在客户端安装包内，无网络环境下亦可独立流畅运行。

---

## 🚀 打包生成 Windows 软件 (.exe / .msi)

你有两种方式可以打包生成 Windows PC 软件：

### 方式一：GitHub Actions 云端全自动打包（推荐，无需本地 Windows 环境）

如果你当前是在 Mac 或 Linux 电脑上开发，不想在本地配置复杂的 Windows 交叉编译环境，可以使用项目中已配置好的 GitHub Actions：

1. 将当前项目初始化为 git 仓库并推送到你的 GitHub（私有或公开仓库均可）：
   ```bash
   git init
   git add .
   git commit -m "feat: init paddleocr desktop"
   git remote add origin <你的GitHub仓库地址>
   git push -u origin main
   ```
2. 进入 GitHub 仓库页面，点击顶部的 **Actions** 标签。
3. 会看到自动触发的 **Build Desktop App** 工作流（也可以手动点击 **Run workflow** 运行）。
4. 编译完成后，在工作流产物（Artifacts）中即可直接下载包含 `.exe` 和 `.msi` 的 Windows 安装包压缩包！

---

### 方式二：在 Windows 电脑上本地直接打包

如果你有一台 Windows 电脑（或 Windows 虚拟机），可以直接在本地打包：

#### 1. 前置环境准备（仅打包电脑需要，使用软件的终端用户不需要）
- **Node.js**：[Node.js 官方下载安装](https://nodejs.org/)（推荐 LTS 版本）
- **Rust**：访问 [rustup.rs](https://rustup.rs/) 下载安装 `rustup-init.exe`，安装时选择默认的 MSVC 工具链（Visual Studio C++ Build Tools）。
- **WebView2**：Windows 10/11 系统通常已内置；若旧系统缺失，运行软件时会自动提示安装。

#### 2. 安装项目依赖并打包
在项目根目录下打开 PowerShell 或 CMD，执行：

```bash
# 1. 安装项目依赖
npm install

# 2. 本地调试运行（开发预览）
npm run tauri dev

# 3. 正式打包为 Windows 安装包
npm run tauri build
```

打包完成后，安装包将生成在：
- NSIS 安装程序（推荐）：`src-tauri/target/release/bundle/nsis/PaddleOCR_1.0.0_x64-setup.exe`
- MSI 安装程序：`src-tauri/target/release/bundle/msi/PaddleOCR_1.0.0_x64_en-US.msi`

---

## 📂 项目工程结构

```
paddle-desktop/
├── package.json                  # Node 项目配置与 Tauri CLI 脚本
├── README.md                     # 本说明文档
├── .gitignore                    # Git 忽略配置
├── .github/
│   └── workflows/
│       └── build.yml             # GitHub Actions Windows 云端一键打包工作流
├── src/                          # 前端全部源码与离线资源
│   ├── index.html                # 单张图片 OCR 识别界面
│   ├── pdf.html                  # PDF 全篇幅 OCR 识别界面
│   ├── models/                   # 6 款离线 ONNX 权重文件
│   ├── npm2/                     # ONNX Runtime Web 与 PaddleOCR.js 核心库
│   └── vendor/                   # PDF.js 核心库与 worker
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
- **页面无缝切换**：单图识别与 PDF 识别页面顶部均自带高可见度切换按钮，随时切换模式。
- **全本地离线运算**：数据绝不经过任何第三方服务器，隐私安全无忧。
