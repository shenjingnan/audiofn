# 参与开发（Contributing）

感谢关注 AudioFn！本文档面向贡献者与开发者，覆盖本地环境搭建、CLI 命令与配置参考、audio.cpp 引擎、测试与发布流程。

> 终端用户文档见 [README](README.md) 与[文档站](docs/)。

## 环境准备

| 工具 | 版本 | 用途 |
| ---- | ---- | ---- |
| Rust | 1.97.1（`rust-toolchain.toml` 固定） | 编译 / 测试 / Lint / Format |
| pnpm | 10.x（`packageManager` 字段固定） | Tauri CLI / 桌面前端 / 文档站依赖管理 |
| Node | 见 `.node-version` | 文档站（Next.js）与前端工具链 |
| 平台依赖 | — | Linux 构建需要 `libasound2-dev`（CLI 麦克风采集）与 `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`（Tauri）；macOS 开箱即用 |

常用命令速查（完整说明见下文各节）：

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test   # 完整检查
```

## 快速开始（CLI）

```bash
# 运行
cargo run
cargo run -- config
cargo run -- completion bash        # 生成 shell 补全（zsh / fish / powershell / elvish 同理）

# ASR
cargo run -- asr install-model      # 下载 ASR 模型（qwen3-asr-0.6b）
cargo run -- asr transcribe --wav rec.wav
cargo run -- asr dictate            # 录音转写（回车 / Ctrl-C 停止）
cargo run -- asr devices

# TTS
cargo run -- tts install-model      # 下载 TTS 模型（qwen3-tts-0.6b）
cargo run -- tts run --text "你好" --reference-wav ref.wav --reference-text "参考文本" --output out.wav
cargo run -- tts voices

# 测试
cargo test
cargo test -- --test-threads=1      # 单线程测试（避免 env 竞争）

# 代码质量检查
cargo fmt --check
cargo clippy -- -D warnings
typos .                             # 拼写检查（CI 同款，typos-cli）

# 覆盖率
cargo tarpaulin
```

## 模型来源与校验

模型**不随代码分发**，由 CLI 的 `install-model` 命令（或桌面应用「模型」页）按 `models/` 清单下载：

- **清单**（随仓库）：
  - `models/manifest.json` — 资产清单：`source / sha256 / license / 文件名`
  - `models/model_registry.json` — 模型库预设：条目 id、适用平台、下载引用
  - `models/verified_registry.json` — 人工验证记录
- **校验**：下载后计算 sha256 与清单比对，**不匹配即删除报错**；先落临时文件再原子移动，避免留下损坏的半截模型
- **幂等**：模型已存在且完整则跳过（`--force` 强制重装）
- **合规**：第三方来源与许可见 `models/THIRD_PARTY_NOTICES.md`
- **位置**：默认安装到 `~/.audiofn/models/<模型名>/`

当前预设（`model_registry.json`）：

| 条目 id | 模型 | 体积 | 适用平台 |
| ------- | ---- | ---- | -------- |
| `asr-qwen3-0.6b-audiocpp` | Qwen3-ASR 0.6B（q8_0，GGUF） | 约 1.1 GB | macOS / Linux |
| `tts-qwen3-06b-base-q8-audiocpp` | Qwen3-TTS 0.6B Base（q8_0） | 约 1.9 GB | macOS (Apple Silicon) |
| `tts-qwen3-17b-base-q8-audiocpp` | Qwen3-TTS 1.7B Base（q8_0） | 约 2.5 GB | macOS (Apple Silicon) |

## CLI 命令参考

### 语音识别（ASR）

Qwen3-ASR-0.6B 离线转写（audio.cpp 引擎，GGUF 单文件）：30 语言自动识别，原生输出标点，
不支持热词。`asr transcribe` 转写 wav 文件（不需要麦克风），`asr dictate` 麦克风录音、
停止后整段转写。

```bash
# 1. 下载模型（约 1.1GB，默认安装到 ~/.audiofn/models/<模型名>，不入库）
cargo run -- asr install-model

# 2. 文件转写（--wav 指定要转写的音频；语种自动识别，也可 --language zh）
cargo run -- asr transcribe --wav rec.wav

# 3. 免提听写：录音 → 回车 / Ctrl-C / --duration 停止 → 整段转写输出全文
cargo run -- asr dictate

# 4. 查看可用麦克风设备
cargo run -- asr devices
```

| 命令 | 说明 |
|------|------|
| `asr transcribe` | 离线转写 wav。`--wav` 文件路径、`--language` 语种（缺省自动识别）、`--model-dir` 模型目录 |
| `asr dictate` | 麦克风录音后整段转写。`--device` 设备、`--duration` 秒数上限、`--language` 语种、`--model-dir` |
| `asr devices` | 列出可用输入设备 |
| `asr install-model` | 下载安装 ASR 模型。`--registry-id` 指定模型库条目、`--force` 强制重装 |

配置（`~/.audiofn/settings.toml` 的 `[asr]` 段，全部可选）：

```toml
[asr]
model_dir = "/path/to/model"   # 模型目录（支持 ${env.VAR}）
model_type = "qwen3_asr"       # 模型族（当前仅 qwen3_asr）
language = "zh"                # 转写语种；缺省由模型自动识别
use_itn = true                 # 反向文本正则化（audiocpp 后端忽略）
provider = "cpu"               # 推理后端（auto 时 macOS 用 Metal，Linux 纯 CPU）
num_threads = 4                # CPU 推理线程数
debug = false
```

### 文本转语音（TTS）

Qwen3-TTS-0.6B / 1.7B Base（audio.cpp 引擎，12Hz，24kHz 输出）零样本音色克隆：
给一段参考音频 + 它的逐字转写，就能用该音色合成任意文本（中英日韩德法俄葡西意 10 语种）。
整段合成、非流式；Base 版没有「自动音色」兜底，**必须提供参考音色**。

```bash
# 1. 下载模型（0.6B 约 1.9GB / 1.7B 约 2.5GB，默认安装到 ~/.audiofn/models/<模型名>）
cargo run -- tts install-model                     # 缺省 0.6B；--registry-id 切 1.7B

# 2. 查看音色库（自定义音色；Base 版需克隆音色）
cargo run -- tts voices

# 3. 克隆合成：参考音频 + 参考文本 → 任意文本
cargo run -- tts run --text "你好" \
  --reference-wav ref.wav --reference-text "参考音频里说的原话" --output out.wav

# 4. 复用音色库里的自定义音色（桌面端录制保存）
cargo run -- tts run --text "你好" --voice <音色id>
```

- **音色库**：自定义音色持久化在 `~/.audiofn/voices/`（wav + manifest.json）。桌面端「音色库」
  支持上传音频或在线录音，参考文本可一键自动转写；CLI `tts voices` 同样列出。
- **输出**：默认 `~/.audiofn/tts/<时间戳>.wav`，`--output` 指定路径
- **语速**：`--speed`（缺省 1.0）

| 命令 | 说明 |
|------|------|
| `tts run` | 合成文本为 wav。`--text` 必填；`--reference-wav/--reference-text` 克隆参考、`--voice` 音色 id、`--speed` 语速、`--output` 输出路径、`--model-dir`/`--engine-path` 覆盖定位 |
| `tts voices` | 列出可用音色（自定义音色库）。`--model-dir` |
| `tts install-model` | 下载安装 TTS 模型。`--registry-id`（缺省 `tts-qwen3-06b-base-q8-audiocpp`）、`--force` |

配置（`~/.audiofn/settings.toml` 的 `[tts]` 段，全部可选）：

```toml
[tts]
model_dir = "/path/to/model"   # 模型目录（支持 ${env.VAR}）
model_type = "qwen3_tts_06"    # qwen3_tts_06 | qwen3_tts_17
backend = "audiocpp"           # 推理后端（当前仅 audiocpp）
engine_path = "/path/to/audiocpp_server"  # 缺省由 locator 自动定位
voice = "my-voice"             # 默认音色 id（音色库）
reference_wav = "/path/ref.wav"    # 默认参考音频（--voice 未指定时）
reference_text = "参考音频转写"    # 默认参考文本
speed = 1.0                    # 语速
provider = "cpu"               # 推理后端（auto 时 macOS 用 Metal，Linux 纯 CPU）
num_threads = 4                # CPU 推理线程数
debug = false
```

### 其它命令

| 命令 | 说明 |
|------|------|
| `config` | 打印运行时配置（版本 / 日志级别） |
| `completion <shell>` | 生成 bash / zsh / fish / powershell / elvish 补全脚本（`hide`，不在 `--help` 列出） |

## audio.cpp sidecar 引擎

ASR / TTS 推理由 [audio.cpp](https://github.com/0xShug0/audio.cpp)（Apache-2.0）引擎完成，
引擎二进制 `audiocpp_server` 作为 Tauri externalBin 随安装包分发，CLI 则由
`src/audiocpp/locator.rs` 按以下顺序自动定位（也可 `--engine-path` / `[tts] engine_path` 显式指定）：
显式路径 → 主程序同目录（Tauri externalBin 落位点）→ `~/.audiofn/engines/` → `PATH`。

`pnpm tauri dev` / `pnpm tauri build` 要求 `src-tauri/binaries/audiocpp_server-<target-triple>`
存在（该目录不入库）：

```bash
# 从本仓库 Release 下载（日常；首次发版前无产物，用下面的 --build）
scripts/fetch-audiocpp-dev.sh

# 本地源码编译（裁剪构建仅含 qwen3_tts + qwen3_asr 两族模型）
scripts/fetch-audiocpp-dev.sh --build
```

- **平台**：仅 macOS / Linux。macOS（Apple Silicon）启用 Metal，Linux 纯 CPU。
- **版本**：引擎版本 pin 在 `.github/workflows/release.yml` 的 `AUDIOCPP_REF`（与
  `tauri.conf.json` 同步审查）。
- **只跑 TTS/ASR 不装引擎**：CLI 侧仅影响推理命令本身；`asr devices`、`config` 等命令不依赖引擎。

## 桌面应用开发（Tauri 2）

复用根 crate 的 ASR / TTS / 模型库 / 音频 / 配置逻辑，代码在 `src-tauri/`，
前端为 React + Vite + TypeScript（Tailwind CSS + shadcn/ui，构建产物打包进应用）。
页面：概览（状态）、模型（ASR 转写 / TTS 合成与音色库）、设置。

```bash
# 安装 Tauri CLI（首次）
pnpm install

# 放置引擎 sidecar（首次跑 tauri dev 前必需）
scripts/fetch-audiocpp-dev.sh

# 开发模式（热重载，需已下载模型：cargo run -- asr install-model / tts install-model）
pnpm tauri dev

# 构建当前平台的安装包（macOS 产出 .app/.dmg）
pnpm tauri build

# 仅检查 / Lint tauri crate（Linux 需 webkit 依赖）
cargo check -p audiofn-app
cargo clippy -p audiofn-app -- -D warnings

# 前端单测 / Lint / 构建（目录 src-tauri/frontend/）
pnpm --dir src-tauri/frontend test:run
pnpm --dir src-tauri/frontend check
pnpm --dir src-tauri/frontend build
```

### 一键重启（开发模式白屏）

设置页提供「重启应用」：退出后自动重新拉起，用于让需要重启才能生效的配置立即生效。

- **打包版（生产）** — 正常：前端资源内置（`asset://`），重启后直接加载。
- **开发模式（`pnpm tauri dev`）** — 重启后新进程会**白屏**。原因：Tauri 内置重启只重新拉起应用二进制、不重跑 `beforeDevCommand`，而 `tauri dev` 在应用退出时会连同 Vite dev server 一起拆掉（[tauri#6163](https://github.com/tauri-apps/tauri/issues/6163)），新进程连不上 `localhost:1420`。需要重启效果时请手动重跑 `pnpm tauri dev`。

## 文档站（docs/）

[fumadocs](https://fumadocs.dev) + Next.js（静态导出），内容在 `docs/content/docs/`。

```bash
pnpm install                       # 首次
pnpm --filter audiofn-docs test    # 平台检测逻辑单测（docs/lib/downloads.test.ts）
pnpm --dir docs build              # 静态导出（CI 校验同款）
```

部署由 Cloudflare Pages Git 集成完成；`.github/workflows/docs.yml` 只做 PR 构建校验。
`docs/lib/downloads.ts` 的下载直链与 README「应用下载」、release.yml 的资产重命名
步骤三方对齐，改动任何一侧需同步核对。

## 项目结构

```
├── Cargo.toml           # workspace 根（crate：audiofn，CLI bin：audiofn-cli）
├── rust-toolchain.toml  # Rust 工具链版本（1.97.1）
├── src/
│   ├── main.rs          # 入口文件
│   ├── lib.rs           # 库入口 + 测试工具（test_util 临时 HOME 隔离）
│   ├── cli.rs           # CLI 命令定义（asr / tts / config / completion）
│   ├── asr/             # 语音识别
│   │   ├── mod.rs       # 门面（run_offline / transcribe_wav / is_installed）
│   │   ├── config.rs    # [asr] 配置解析与默认值
│   │   ├── offline.rs   # wav 文件转写
│   │   └── dictate.rs   # 免提听写（录音 → 停止 → 整段转写）
│   ├── tts/             # 语音合成
│   │   ├── mod.rs       # 门面（TtsEngine / 合成 / 默认输出路径）
│   │   ├── config.rs    # [tts] 配置解析与默认值 + 预检
│   │   ├── voice.rs     # 音色参数解析（内置音色 / 自定义参考）
│   │   ├── voice_store.rs # 自定义音色库（~/.audiofn/voices/）
│   │   └── reaction.rs  # 可插拔结果反应（控制台 / 测试）
│   ├── audiocpp/        # audio.cpp sidecar 客户端
│   │   ├── locator.rs   # 引擎二进制定位（engine-path / engines 目录 / PATH / Tauri 资源）
│   │   ├── server.rs    # sidecar 进程生命周期（启动 / 就绪 / 清理）
│   │   ├── client.rs    # SSE 流式客户端（合成 / 转写请求）
│   │   ├── families.rs  # TTS 模型族表（qwen3_tts_06 / 17）
│   │   ├── asr_families.rs # ASR 模型族表（qwen3_asr）
│   │   └── provider.rs  # 引擎能力探测
│   ├── model_library/   # 模型库核心服务
│   │   ├── registry.rs  # model_registry.json 预设 + 平台过滤
│   │   ├── asset.rs     # 下载 / sha256 校验 / 原子落位 / 进度
│   │   ├── install.rs   # 安装编排
│   │   ├── catalog.rs   # 已安装模型扫描（~/.audiofn/models/）
│   │   ├── storage.rs   # 库元数据（.audiofn-lib.json）
│   │   └── verified.rs  # verified_registry.json
│   ├── audio.rs         # cpal 麦克风采集 + 重采样 + 设备枚举
│   ├── config/
│   │   ├── mod.rs       # 配置模块入口
│   │   ├── settings.rs  # ~/.audiofn/settings.toml（[asr]/[tts]/[model_library]/[shortcuts]）
│   │   └── shortcuts.rs # 全局快捷键
│   ├── logging.rs       # tracing 双层日志（文件 + stderr）
│   └── datetime.rs      # 日期时间工具
├── models/              # 模型清单（本体不入库，按清单下载）
│   ├── manifest.json    # 资产清单（source / sha256 / license / 文件名）
│   ├── model_registry.json # 模型库预设
│   ├── verified_registry.json # 人工验证记录
│   └── THIRD_PARTY_NOTICES.md
├── src-tauri/           # Tauri 2 桌面应用（workspace 成员）
│   ├── src/lib.rs       # Tauri commands + 听写 / 合成线程
│   ├── frontend/        # React + Vite + TypeScript 控制面板（Tailwind + shadcn/ui）
│   ├── tauri.conf.json  # Tauri 配置（打包目标 / externalBin / 权限文案）
│   ├── capabilities/    # 权限声明
│   └── icons/           # 应用图标
├── docs/                # fumadocs 文档站（Next.js + MDX）
├── tests/               # 集成测试
├── package.json         # Tauri CLI（@tauri-apps/cli）
├── scripts/             # 引擎获取 / dmg 修复注入 / 图标生成等脚本
├── .github/             # CI / 发布 / 网盘上传流水线
└── .githooks/           # Git hooks
```

## 依赖说明

| 分类 | Crate | 用途 |
|------|-------|------|
| 核心 | clap / clap_complete | CLI 参数解析 / Shell 补全生成 |
| 核心 | tokio | 异步运行时 |
| 核心 | serde / serde_json / toml | 序列化 |
| 核心 | chrono | 日期时间处理 |
| 核心 | tracing / tracing-subscriber | 日志 |
| 核心 | thiserror / anyhow | 错误处理 |
| 音频 | cpal | 麦克风采集与设备枚举 |
| 音频 | hound | wav 解码 / 编码（sidecar 返回的 wav → f32 样本、听写落盘） |
| 音频 | objc2-av-foundation | macOS 麦克风权限请求（仅 macOS target） |
| 引擎 | reqwest | audio.cpp sidecar HTTP/SSE 客户端 |
| 引擎 | base64 | sidecar SSE 流式 delta 载荷解码 |
| 引擎 | sysinfo | sidecar 残留进程按 pid 检查与清理 |
| 模型下载 | ureq | HTTP 客户端（流式下载 + socks 代理 + 系统证书） |
| 模型下载 | sha2 / hex | 下载模型的 sha256 校验 |
| 测试 | tempfile / tiny_http | 临时 HOME 隔离 / SSE stub server |

## 发布流程

每次发布新版本会自动构建 **macOS（Intel + Apple Silicon）/ Linux** 安装包并合并到一个 GitHub Release：

1. 合入 `main` 后，`publish.yml` 中的 release-plz 自动 bump 版本、更新 changelog，打出 `vX.Y.Z` tag 并发布到 crates.io，同时维护「版本发布 PR」。
2. tag push 触发 `release.yml`：先从 `AUDIOCPP_REF` 源码编译 audio.cpp 引擎 sidecar，再由 `tauri-action` 在 macOS（双架构）/ Linux 原生 runner 上构建安装包（`.dmg` / `.deb` / `.rpm` / `.AppImage`）。macOS 的 `.dmg` 在上传前由 `scripts/patch-dmg-gatekeeper.sh` 注入「首次打开修复.command」——未签名应用被 Gatekeeper 报「已损坏」，用户双击该脚本即可自动安装 + 修复（详见 README「macOS 首次打开」）。
3. 安装包统一重命名为固定资产名（跨版本不变，README 的 `releases/latest/download/` 直链因此永远指向最新版）：`AudioFn_macOS_arm64.dmg` / `AudioFn_macOS_x64.dmg` / `AudioFn_Linux_amd64.deb` / `AudioFn_Linux_x86_64.rpm` / `AudioFn_Linux_amd64.AppImage`。
4. 构建成功后自动发布为正式 Release（`draft: false`），`release: published` 事件再触发 `upload-baidu-pan.yml` 把安装包与安装说明上传百度网盘（安装说明由 README「应用下载」章节经 awk 提取，**该二级标题不可改名**）。

发布产物矩阵：

| 平台 | 安装包 |
|------|--------|
| macOS 13+ (Apple Silicon) | `.dmg` |
| macOS 13+ (Intel) | `.dmg` |
| Linux x86_64 | `.deb` + `.rpm` + `.AppImage` |

> 签名：当前为未签名构建，适合内部/测试分发。正式对外发布时在仓库 Secrets 配置
> Apple Developer ID 证书（`APPLE_SIGNING_IDENTITY / APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID`），
> tauri-action 会自动签名/公证 macOS 产物。

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
