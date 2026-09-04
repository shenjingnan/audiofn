# CLAUDE.md - AudioFn

本文档为 Claude Code 提供项目上下文和开发规范。

## 项目概述

**AudioFn** 是一个开源的本地优先桌面 ASR & TTS 工具（An open-source, local-first desktop
ASR & TTS toolkit with voice cloning）：Qwen3-ASR 离线转写 + Qwen3-TTS 音色克隆，
提供 Tauri 2 桌面 GUI 与 `audiofn asr / tts` 命令行。推理由 audio.cpp sidecar
引擎（`audiocpp_server`，ggml）完成，模型本地运行，数据不出设备。

- **ASR**：Qwen3-ASR-0.6B 离线转写（30 语言自动识别），`asr transcribe`（文件）与
  `asr dictate`（录音 → 停止后整段转写）
- **TTS**：Qwen3-TTS-0.6B / 1.7B 零样本音色克隆（参考音频 + 参考文本 → 任意文本），
  自定义音色持久化在 `~/.audiofn/voices/`
- **模型库**：`models/` 三个清单（manifest / registry / verified），模型本体不入库，
  按 sha256 校验下载到 `~/.audiofn/models/`
- **平台**：macOS 13+（arm64 / x86_64）、Linux x86_64；**无 Windows**

## 技术栈

| 技术           | 版本  | 用途                         |
| -------------- | ----- | ---------------------------- |
| Rust           | 1.97 | 编程语言 / 编译 / 测试 / Lint / Format |
| clap           | 4.x   | CLI 参数解析                 |
| tokio          | 1.x   | 异步运行时                   |
| audio.cpp sidecar | 0.7.x（`AUDIOCPP_REF`） | ASR / TTS 推理引擎（ggml，`audiocpp_server` 子进程，SSE 流式协议） |
| cpal           | 0.18  | 麦克风采集 + 重采样          |
| serde          | 1.x   | JSON/TOML 序列化/反序列化    |
| tracing        | 0.1   | 日志和诊断                   |
| Tauri          | 2.x   | 桌面应用框架（workspace 成员 `src-tauri/`） |
| React + Vite   | 19.x  | 桌面 GUI 前端（`src-tauri/frontend/`，Tailwind + shadcn/ui） |

## 快速命令参考

```bash
# 开发
cargo run                           # 直接运行（无参进入帮助）
cargo run -- config                 # 显示配置
cargo run -- completion bash        # 生成 shell 补全

# ASR（Qwen3-ASR，离线转写）
cargo run -- asr install-model      # 下载 ASR 模型（qwen3-asr-0.6b，约 1.1GB）
cargo run -- asr transcribe         # 转写 wav（缺省用模型自带示例音频）
cargo run -- asr dictate            # 录音转写（回车 / Ctrl-C 停止）
cargo run -- asr devices            # 列出输入设备

# TTS（Qwen3-TTS，音色克隆）
cargo run -- tts install-model      # 下载 TTS 模型（qwen3-tts-0.6b）
cargo run -- tts run --text "你好" --reference-wav ref.wav --reference-text "参考文本" --output out.wav
cargo run -- tts voices             # 音色库（模型包内置 + 自定义音色）

# 测试
cargo test                          # 运行测试
cargo test -- --test-threads=1      # 单线程测试（避免 env 竞争）

# 代码质量
cargo fmt                           # 格式化代码
cargo fmt --check                   # 格式检查
cargo clippy                        # Lint 检查
cargo clippy -- -D warnings         # 严格 Lint 检查
cargo fmt --check && cargo clippy -- -D warnings && cargo test   # 完整检查

# 桌面应用（Tauri 2，位于 src-tauri/，path 依赖根 crate 库）
pnpm install                        # 首次：安装 @tauri-apps/cli
scripts/fetch-audiocpp-dev.sh       # 首次跑 tauri dev 前：放置 audio.cpp sidecar（--build 走源码编译）
pnpm tauri dev                      # 开发模式（转写 / 合成 / 音色库 / 设置面板）
pnpm tauri build                    # 构建当前平台安装包（macOS: .app/.dmg）
cargo check -p audiofn-app          # 仅检查 tauri crate（Linux 需 webkit 依赖）
cargo clippy -p audiofn-app -- -D warnings   # tauri crate Lint

# 构建与文档
cargo build                         # 调试构建（默认只构建根 CLI crate）
cargo build --release               # 发布构建
cargo doc --open                    # 生成并打开 API 文档
cargo tarpaulin                     # 生成覆盖率报告
pnpm --dir docs build               # 构建文档站（Next.js 静态导出）
```

## 代码风格规范

由 `cargo fmt` 和 `cargo clippy` 强制执行（Rust Edition 2024）：

- **缩进**: 2 空格
- **行宽**: 最大 100 字符

### 命名约定

| 类型      | 约定                 | 示例           |
| --------- | -------------------- | -------------- |
| 文件      | snake_case           | `my_module.rs` |
| 类/结构体 | PascalCase           | `MyStruct`     |
| 函数/变量 | snake_case           | `my_function`  |
| 常量      | SCREAMING_SNAKE_CASE | `MAX_COUNT`    |
| 类型/trait| PascalCase           | `UserConfig`   |
| 枚举      | PascalCase           | `ModelRole`    |

## 项目结构

```
├── Cargo.toml           # workspace 根（crate：audiofn，CLI bin：audiofn-cli）
├── rust-toolchain.toml  # Rust 工具链版本（1.97.1）
├── src/
│   ├── main.rs          # 入口文件
│   ├── lib.rs           # 库入口 + 测试工具（test_util 临时 HOME 隔离）
│   ├── cli.rs           # CLI 命令定义（asr / tts / config / completion）
│   ├── asr/             # 语音识别（mod 门面 / config / offline 文件转写 / dictate 听写）
│   ├── tts/             # 语音合成（mod 门面 / config / voice 音色解析 / voice_store 自定义音色库）
│   ├── audiocpp/        # audio.cpp sidecar 客户端（locator 定位 / server 生命周期 / client SSE / 模型族表）
│   ├── model_library/   # 模型库（registry 预设 / asset 下载校验 / install 安装 / catalog 扫描）
│   ├── audio.rs         # cpal 麦克风采集 + 重采样
│   ├── config/          # settings.toml 配置 + shortcuts
│   ├── logging.rs       # tracing 双层日志（文件 + stderr）
│   └── datetime.rs      # 日期时间工具
├── models/              # 模型清单（本体不入库，按清单下载）
│   ├── manifest.json    # 资产清单（source / sha256 / license）
│   ├── model_registry.json # 模型库预设（id / 平台 / 下载引用）
│   ├── verified_registry.json # 人工验证记录
│   └── THIRD_PARTY_NOTICES.md
├── src-tauri/           # Tauri 2 桌面应用（workspace 成员）
│   ├── src/lib.rs       # Tauri commands + 听写 / 合成线程
│   ├── frontend/        # React + Vite + TypeScript 控制面板（Tailwind + shadcn/ui）
│   ├── tauri.conf.json  # Tauri 配置（打包 / externalBin / 权限文案）
│   ├── capabilities/    # 权限声明
│   └── icons/           # 应用图标
├── docs/                # fumadocs 文档站（Next.js + MDX）
├── tests/               # 集成测试
├── package.json         # Tauri CLI（@tauri-apps/cli）
├── scripts/             # 引擎获取 / dmg 修复注入 / 图标生成等脚本
├── .github/             # CI / 发布 / 网盘上传流水线
└── .githooks/           # Git hooks
```

用户数据目录：`~/.audiofn/`（`settings.toml`、`models/`、`voices/`、`tts/` 输出、日志）。

## 发布流程（桌面安装包）

`release-plz` 负责版本/tag/changelog/crates.io；push `vX.Y.Z` tag 后由
`.github/workflows/release.yml`（tauri-action）在 macOS（双架构）/ Linux 原生 runner
构建安装包并附到正式 Release。macOS dmg 在上传前由 `scripts/patch-dmg-gatekeeper.sh`
注入「首次打开修复.command」，安装包统一重命名为
`AudioFn_macOS_arm64.dmg / AudioFn_macOS_x64.dmg / AudioFn_Linux_amd64.deb /
AudioFn_Linux_x86_64.rpm / AudioFn_Linux_amd64.AppImage`——README「应用下载」直链与
docs 站 `docs/lib/downloads.ts` 必须与这些资产名逐字一致。

**维护契约**：README 的二级标题「## 应用下载」是 `.github/workflows/upload-baidu-pan.yml`
用 awk 提取安装说明的起点，改名会让网盘发布流程 fail-fast。

详见 CONTRIBUTING.md「发布流程」。

## 已知限制

- **一期不做 VAD / 长音频切分**：听写与文件转写整段送模型，超长音频内存占用大。
- **TTS 整段合成、非流式**：一次请求合成完整文本，长文本耗时随长度线性增长。
- **Linux 纯 CPU**：Linux 构建未启用 GPU 后端；macOS（Apple Silicon）走 Metal。
- **无 Windows**：构建与安装包白名单只有 macOS / Linux。
- **开发模式重启会白屏**：`pnpm tauri dev` 下点击「重启」后新进程白屏。根因是 Tauri
  内置重启不重跑 `beforeDevCommand`，而 `tauri dev` 在应用退出时拆掉 Vite dev server
  （[tauri#6163](https://github.com/tauri-apps/tauri/issues/6163)），新进程连不上
  `localhost:1420`。生产打包版正常。详见 CONTRIBUTING.md「一键重启」。

## Git 工作流

### 分支命名

- `feature/xxx` - 新功能
- `fix/xxx` - Bug 修复
- `docs/xxx` - 文档更新
- `refactor/xxx` - 重构

### Commit 规范

遵循 [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]
```

**类型**:

- `feat` - 新功能
- `fix` - Bug 修复
- `docs` - 文档更新
- `style` - 代码格式
- `refactor` - 重构
- `perf` - 性能优化
- `test` - 测试相关
- `chore` - 构建/工具
