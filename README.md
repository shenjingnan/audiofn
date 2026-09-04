<div align="right">

**简体中文** | [English](README.en.md)

</div>

<div align="center">
  <img src="docs/public/logo.svg" alt="AudioFn Logo" width="300" />

  <p>
    <a href="https://github.com/shenjingnan/audiofn/releases"><img src="https://img.shields.io/github/v/release/shenjingnan/audiofn" alt="GitHub Release" /></a>
    <a href="https://crates.io/crates/audiofn"><img src="https://img.shields.io/crates/v/audiofn" alt="crates.io 版本" /></a>
    <a href="https://crates.io/crates/audiofn"><img src="https://img.shields.io/crates/d/audiofn" alt="crates.io 下载量" /></a>
    <a href="https://github.com/shenjingnan/audiofn/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/shenjingnan/audiofn/ci.yml?branch=main&label=CI" alt="GitHub Actions CI 状态" /></a>
    <a href="https://codecov.io/gh/shenjingnan/audiofn"><img src="https://codecov.io/gh/shenjingnan/audiofn/graph/badge.svg" alt="Codecov 覆盖率" /></a>
    <br />
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPL--3.0--only-blue" alt="License: GPL-3.0-only" /></a>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.97%2B-dea584?logo=rust" alt="Rust 1.97+" /></a>
    <a href="#应用下载"><img src="https://img.shields.io/badge/macOS-000000?logo=apple&logoColor=white" alt="macOS 支持" /></a>
    <a href="#应用下载"><img src="https://img.shields.io/badge/Linux-FCC624?logo=linux&logoColor=black" alt="Linux 支持" /></a>
  </p>
</div>

**AudioFn** — An open-source, local-first desktop ASR & TTS toolkit with voice cloning.

开源的本地优先桌面 ASR & TTS 工具：离线语音转写 + 音色克隆，全部在本地运行，数据不出设备。

## ✨ 特性一览

- **语音识别（ASR）** — Qwen3-ASR-0.6B 离线转写，30 语言自动识别，无需联网；CLI `audiofn asr` 与桌面「转写」页（文件转写 / 录音听写）两种用法
- **语音合成（TTS）** — Qwen3-TTS-0.6B / 1.7B 零样本音色克隆：录一段 5–30 秒参考音频 + 对应文本，即可用该音色合成任意文本（支持中英日韩等 10 语种）
- **桌面 + CLI 双形态** — Tauri 2 桌面面板（概览 / 模型库 / 转写 / 合成 / 音色库 / 设置）+ `audiofn asr` / `audiofn tts` 命令行
- **本地优先** — audio.cpp sidecar 引擎（ggml；macOS Metal / Linux CPU），模型一键下载，音频与文本全程不出设备
- **模型库** — 应用内「模型」页一键下载、sha256 校验、切换当前模型；模型本体不入库，清单（来源 / 校验和 / 许可）随仓库版本管理
- **平台** — macOS 13+（Apple Silicon / Intel）、Linux x86_64（deb / rpm / AppImage）；**不提供 Windows 版本**

## 应用下载

点击下方按钮直接下载对应系统的最新版安装包（无需登录 GitHub，自动指向最新 Release）：

| 系统 | 芯片 / 架构 | 立即下载 |
| --- | --- | --- |
| macOS 13+ | Apple Silicon（M1/M2/M3/M4） | [![立即下载](https://img.shields.io/badge/%E7%AB%8B%E5%8D%B3%E4%B8%8B%E8%BD%BD-8E8E93?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/shenjingnan/audiofn/releases/latest/download/AudioFn_macOS_arm64.dmg) |
| macOS 13+ | Intel | [![立即下载](https://img.shields.io/badge/%E7%AB%8B%E5%8D%B3%E4%B8%8B%E8%BD%BD-8E8E93?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/shenjingnan/audiofn/releases/latest/download/AudioFn_macOS_x64.dmg) |
| Ubuntu / Debian | amd64 | [![立即下载](https://img.shields.io/badge/%E7%AB%8B%E5%8D%B3%E4%B8%8B%E8%BD%BD-A80030?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/shenjingnan/audiofn/releases/latest/download/AudioFn_Linux_amd64.deb) |
| Fedora / RHEL | x86_64 | [![立即下载](https://img.shields.io/badge/%E7%AB%8B%E5%8D%B3%E4%B8%8B%E8%BD%BD-294172?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/shenjingnan/audiofn/releases/latest/download/AudioFn_Linux_x86_64.rpm) |

- Linux 也可选 [AppImage](https://github.com/shenjingnan/audiofn/releases/latest/download/AudioFn_Linux_amd64.AppImage) 免安装直接运行。
- 完整版本与更新日志见 [Releases](https://github.com/shenjingnan/audiofn/releases)。
- 🍎 Mac 不确定芯片？左上角  →「关于本机」：显示「芯片：Apple M…」选 arm64，显示「处理器：Intel…」选 x64。
- 📦 模型不随安装包分发：首次使用在应用「模型」页一键下载（ASR 约 1.1GB、TTS 约 1.9GB 起）。

### macOS 首次打开（未签名）

项目未申请 Apple Developer 证书，安装包**未签名**。首次打开会被 Gatekeeper 拦截，提示「"AudioFn" 已损坏，无法打开。你应该将它移到废纸篓。」——**并非真的损坏**，只是系统给下载的文件加了隔离属性。两种处理方式：

- **双击修复脚本（推荐）**：打开下载的 dmg，双击其中的「**首次打开修复.command**」。脚本会自动把 AudioFn 安装到「应用程序」、清除隔离属性并启动，无需手动拖动（若双击时提示「无法验证开发者」，右键该文件 →「打开」→ 再次点击「打开」）。
- **手动执行命令**：先把 AudioFn 拖入「应用程序」，再打开「终端」（Terminal）执行：

  ```bash
  xattr -cr "/Applications/AudioFn.app"
  ```

若 App 不在「应用程序」，把命令里的路径换成实际位置；或右键 App →「打开」→ 再次点击「打开」。

## 快速上手（CLI）

```bash
# 下载模型（模型不入库，默认安装到 ~/.audiofn/models/）
cargo run -- asr install-model                     # 下载 ASR 模型（qwen3-asr-0.6b）
cargo run -- tts install-model                     # 下载 TTS 模型（qwen3-tts-0.6b）

# 语音识别
cargo run -- asr transcribe --wav rec.wav          # 文件转写（自动识别语种）
cargo run -- asr dictate                           # 录音转写（回车或 Ctrl-C 停止）
cargo run -- asr devices                           # 列出输入设备

# 语音合成（音色克隆）
cargo run -- tts run --text "你好" --reference-wav ref.wav --reference-text "参考文本" --output out.wav
cargo run -- tts voices                            # 音色库（模型包内置 + 自定义音色）

# 其它
cargo run -- config                                # 显示配置
cargo run -- completion bash                       # 生成 Shell 补全（zsh / fish / powershell / elvish 同理）
```

音色克隆三步：录一段 5–30 秒干净的参考音频 → 写下音频里说的原话（逐字）→ `--reference-wav` + `--reference-text` 一起传给 `tts run`。音色也可以在桌面端「音色库」录制保存，之后用 `--voice <音色id>` 复用。

## 参与开发

```bash
# 开发
cargo run                          # 直接运行（无参进入帮助）
cargo build                        # 调试构建
cargo test                         # 运行测试

# 代码质量（完整检查，CI 同款）
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

- [贡献指南](CONTRIBUTING.md)：环境搭建、CLI 命令与配置参考、audio.cpp 引擎、项目结构与发布流程
- [文档站](docs/)：介绍、桌面端、CLI、模型库、音色克隆指南与常见问题
- 桌面应用开发：`pnpm install` → `scripts/fetch-audiocpp-dev.sh`（放置引擎 sidecar）→ `pnpm tauri dev`

## 项目结构

```
├── Cargo.toml           # workspace 根（crate：audiofn，CLI bin：audiofn-cli）
├── rust-toolchain.toml  # 固定工具链 1.97.1
├── src/
│   ├── main.rs          # 入口文件
│   ├── lib.rs           # 库入口 + 测试工具（test_util 临时 HOME 隔离）
│   ├── cli.rs           # CLI 命令定义（asr / tts / config / completion）
│   ├── asr/             # 语音识别（Qwen3-ASR：offline 文件转写 + dictate 听写）
│   ├── tts/             # 语音合成（Qwen3-TTS：合成 + 音色解析 + 自定义音色库）
│   ├── audiocpp/        # audio.cpp sidecar 客户端（引擎定位 / 生命周期 / SSE 解析）
│   ├── model_library/   # 模型库（registry 预设 / 下载 / sha256 校验 / 安装 / 切换）
│   ├── audio.rs         # cpal 麦克风采集 + 重采样
│   ├── config/          # settings.toml 配置 + 快捷键
│   ├── logging.rs       # tracing 双层日志（文件 + stderr）
│   └── datetime.rs      # 日期时间工具
├── models/              # 模型清单（source / sha256 / license，模型本体不入库）
├── src-tauri/           # Tauri 2 桌面应用（workspace 成员）
│   ├── src/lib.rs       # Tauri commands + 听写 / 合成线程
│   ├── frontend/        # React + Vite + TypeScript 面板（Tailwind + shadcn/ui）
│   ├── tauri.conf.json  # Tauri 配置（打包 / externalBin / 权限文案）
│   ├── capabilities/    # 权限声明
│   └── icons/           # 应用图标
├── docs/                # fumadocs 文档站（Next.js + MDX）
├── tests/               # 集成测试
├── scripts/             # 引擎获取 / dmg 修复注入 / 图标生成等脚本
├── .github/             # CI / 发布 / 网盘上传流水线
└── .githooks/           # Git hooks
```

## 已知限制

- **一期不做 VAD / 长音频切分**：听写与文件转写都是整段送模型，超长音频（小时级）会占大量内存，建议先自行切段。
- **TTS 整段合成、非流式**：一次请求合成完整文本，无边合成边播放；长文本耗时随长度线性增长。
- **Linux 纯 CPU**：Linux 构建的引擎未启用 GPU 后端，推理走 CPU；macOS（Apple Silicon）走 Metal。
- **不提供 Windows 版本**：构建与安装包白名单只有 macOS / Linux。

## 许可

[GPL-3.0-only](LICENSE)
