# audiofn 一期设计：基于 Qwen3-TTS-0.6B 的音色克隆 ASR/TTS 工具

> 状态：已评审通过（2026-09-04）
> 前提：本仓库由 zapmomo 模板克隆而来，目标产品为 **audiofn**——ASR + TTS 处理工具
> （桌面客户端 + CLI），支持 macOS 与 Linux。

## 1. 背景与目标

audiofn 一期目标：**使用 Qwen3-TTS-0.6B 实现音色克隆**，并具备基于 Qwen3-ASR-0.6B
的语音转写能力。以 zapmomo 为模板，复用其模型分发管线、audiocpp sidecar 集成、
桌面 GUI 骨架与 CLI 框架，移除全部「AI 伴侣」能力。

## 2. 概念澄清

| 名称 | 本质 | 音色克隆 |
| --- | --- | --- |
| Qwen3-0.6B | 纯文本 LLM，无音频输入/输出 | 不可行 |
| **Qwen3-TTS-12Hz-0.6B-Base** | 阿里官方开源 TTS（Apache 2.0），3 秒参考音频克隆，10 语言 | **一期采用** |

## 3. 可行性依据（模板中已存在的能力）

- 发布管线已含引擎：`.github/workflows/release.yml:176-178` 构建白名单
  `omnivoice,voxcpm2,qwen3_tts,qwen3_asr`（macOS/Linux 正式包）
- 模型资产已登记：`models/manifest.json:90-120`（qwen3-tts 0.6B/1.7B、qwen3-asr 0.6B，GGUF q8_0）
- Rust 集成已完成：`src/audiocpp/families.rs:111`（QWEN3_TTS_06B，24kHz）、
  `src/audiocpp/client.rs:347`（`apply_voice_fields`：`voice_ref` + `reference_text` 克隆通道）
- CLI 已可用：`tts run --backend audiocpp --reference-wav ... --reference-text ...`（`src/cli.rs:234-255`）
- 实测数据：`docs/plans/2026-08-26-qwen3-tts-audiocpp-integration.md` 记录 E2E RTF 0.72；
  Base 版必须提供参考音频（`VoiceSemantics::ReferenceCloneRequired`）
- 配套已评审：音色上传/备份（`COMPANION_VOICE_UPLOAD_DESIGN.md`）、本地音色库（`src/tts/voice_store.rs`）

**结论：一期工程量主体是「减法裁剪 + 品牌迁移」而非「从零实现」。**

## 4. 总体架构

```
┌─────────────────┐   ┌──────────────────────┐
│  CLI (audiofn)  │   │ 桌面 (audiofn-app)   │
│  asr / tts 子命令 │   │  Tauri 2 + React 面板  │
└────────┬────────┘   └──────────┬───────────┘
         └────────┬──────────────┘
          共享核心 crate（src/）
     model_library ─ audiocpp(client/families)
     tts/voice_store ─ config ─ logging
                  │ spawn 子进程 + 随机端口
                  ▼
        audiocpp_server（本地 HTTP sidecar）
        /v1/audio/speech          ← qwen3_tts 克隆合成
        /v1/audio/transcriptions  ← qwen3_asr 转写
                  ▼
        ggml 后端：macOS Metal（arm64）/ Linux CPU（CUDA 二期）
```

- 引擎构建白名单收窄为 `qwen3_tts,qwen3_asr`（`release.yml` 与
  `scripts/fetch-audiocpp-dev.sh` 同步修改）
- 模型资产只保留：qwen3-tts-0.6b（约 2GB）、qwen3-asr-0.6b（约 1.1GB）、
  qwen3-tts-1.7b（可选，默认不推荐）
- 数据目录 `~/.zapmomo` → `~/.audiofn`；平台收敛 macOS + Linux（移除 Windows）

## 5. 模块裁剪清单

### 删除

| 目标 | 内容 |
| --- | --- |
| `src/kws/`、`src/speaker/`、`src/voice/`、LLM 模块 | 伴侣能力（KWS/声纹/会话/LLM 对话）+ Live2D 角色 |
| `src/asr/`、`src/tts/` 中 sherpa 后端 | Zipformer/SenseVoice/Whisper/ZipVoice/标点/VAD 相关代码 |
| `src-tauri/src/lib.rs` 中角色/LLM/KWS/声纹 commands 与监听线程 | 8941 行预计裁掉过半 |
| 前端页面：角色、Live2D、LLM 配置、唤醒词、声纹、会话 | 保留 设置 / 模型库 / TTS+音色库 / 转写 |
| `manifest.json` 中 9 项资产 | KWS、流式 Zipformer、标点、ZipVoice+Vocos、Silero VAD、omnivoice、voxcpm2、CAM++ |
| `tauri.windows.conf.json`、release.yml Windows 腿、fetch 脚本 Windows 分支 | 平台收敛 |
| `COMPANION_*.md` 三份设计文档 | 随能力移除 |

### 保留并改造

- `src/audiocpp/`：`families.rs` 删 omnivoice/voxcpm2；`asr_families.rs` 只留 qwen3_asr
- `src/model_library/`：manifest 裁剪，下载/校验/安装逻辑不动
- `src/asr/mod.rs`：**补齐缺口**——`transcribe_wav`（`asr/mod.rs:310`）接 audiocpp 后端
- `src/tts/`：audiocpp 后端 + `voice_store.rs` 音色库原样保留
- `src/audio.rs`（cpal 采集）：保留，供「录音 → 转写」

## 6. 一期功能规格

### 音色克隆（核心）

```
GUI：音色库页「录制/上传参考音频 + 对应文本」→ 存 ~/.audiofn/voices/
     合成页「选音色 → 输入文本 → 合成 → 播放/导出 wav」
CLI：audiofn tts run --text "你好" --voice myvoice --output out.wav
     audiofn tts run --text "你好" --reference-wav a.wav --reference-text "原文本"
     audiofn tts voices / audiofn tts install-model
```

- 0.6B Base 约束固化到交互：必须提供参考音频 + 参考文本，无参考时给出引导
- 参考音频规格：wav、mono、16bit；时长建议 5–30 秒
- 整段合成 + 进度提示，不承诺实时首包（`supports_streaming: false`）

### ASR 转写

```
GUI：转写页「选 wav 文件 或 按住录音」→ 结果展示/复制/导出
CLI：audiofn asr transcribe rec.wav [--output out.txt]
     audiofn asr dictate（录音 → 转写，复用 cpal 采集）
     audiofn asr install-model
```

- qwen3-asr-0.6b 自动识别 30 种语言
- 一期不做 VAD：录音/短音频整段直送；超长文件提示「建议 ≤10 分钟」，不硬拦截

### 桌面端一期页面

模型库（下载/引擎状态）、转写、合成+音色库、设置。

## 7. 关键风险

1. **品牌迁移波及面**：crate 名、数据目录、Tauri identifier、crates.io 包名、README/CI。
   策略：小步走——先删 `Cargo.toml` 依赖，让编译器暴露引用点，每步可编译、可提交
2. **首次下载 3GB+ 模型**：下载管线需回归验证（sha256、进度）
3. **Linux 纯 CPU 合成速度**：RTF 0.72 来自模板文档且硬件未知；一期验收安排真机基准
4. **audio.cpp 上游耦合**：白名单收窄后 sidecar（v0.7.1）config/端点需重新冒烟
5. **合规文档**：重写 `THIRD_PARTY_NOTICES.md`（修正漏列 qwen3_asr 的漂移）

## 8. 分阶段实施与验收

| 阶段 | 内容 | 验收标准 |
| --- | --- | --- |
| 1 | 品牌迁移 + 依赖/平台裁剪（sherpa、Windows、伴侣模块） | `cargo fmt --check && cargo clippy -- -D warnings && cargo test` 全绿；`pnpm tauri dev` 可启动 |
| 2 | 引擎白名单收窄 + manifest 裁剪 + `transcribe_wav` 接 audiocpp | CLI 端到端：`install-model` → `tts run`（克隆）→ `asr transcribe` |
| 3 | GUI 裁剪：模型库/转写/合成+音色库/设置 四页 | GUI 端到端：录参考音频 → 保存音色 → 合成播放；录音 → 转写 |
| 4 | release.yml（macOS/Linux）+ README/docs 重写 | 双平台安装包可安装，真机冒烟通过 |
