# audiofn 一期改造实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把 zapmomo 模板裁剪为 audiofn——sherpa-free 的 ASR+TTS 工具（Qwen3-TTS-0.6B 音色克隆 + Qwen3-ASR 转写），macOS/Linux。

**Architecture:** 全部 AI 能力收敛到 audiocpp sidecar（本地 HTTP，`/v1/audio/speech` + `/v1/audio/transcriptions`）；根 crate 为共享核心 + CLI，src-tauri 为桌面壳；模型资产只留 qwen3-tts-0.6b/1.7b 与 qwen3-asr-0.6b。

**Tech Stack:** Rust 1.97 (edition 2024)、Tauri 2 + React 19 + Vite、audio.cpp sidecar v0.7.1（ggml/Metal）。

**设计依据:** `docs/plans/2026-09-04-audiofn-phase1-design.md`（已评审通过）。

---

## 全局约束（每个任务都必须遵守）

1. **每任务一个 commit，commit 前必须可编译**：
   `cargo fmt && cargo clippy --workspace -- -D warnings && cargo test` 全绿
   （前端任务改为：`pnpm --dir src-tauri/frontend lint && pnpm --dir src-tauri/frontend test`）
2. Commit 遵循 Conventional Commits（`refactor:` / `feat:` / `chore:` / `docs:`）
3. 删除型任务用**编译器驱动**：先删源头，按 `cargo check` 报错逐个清理引用
4. 禁止保留死代码/注释掉的代码块；删除即彻底删除
5. 工具链固定 1.97.1（rust-toolchain.toml），不要动
6. **迁移陷阱**（来自调查，务必记住）：
   - `src/kws/model.rs` 是共享下载基础设施（0 处 sherpa 引用），必须迁走而不是删
   - `src/kws/config.rs` 的 `kws_files_present` 被 `model_library/mod.rs:619,876` 使用，需一并迁走
   - `src/audio.rs:134-149` 的 `Resampler` 包装 `sherpa_onnx::LinearResampler`，是删 sherpa-onnx 依赖的硬阻塞点
   - `~/.zapmomo` 数据目录常量唯一定义点：`src/config/settings.rs:12`

---

# 阶段 1：Rust 后端裁剪（sherpa-free + 伴侣能力移除）

### Task 1: 迁移 kws/model.rs → model_library/asset.rs

**Files:**
- Create: `src/model_library/asset.rs`（内容 = `src/kws/model.rs` 原文，模块文档改为「模型资产下载基础设施」）
- Modify: `src/kws/config.rs` → 把 `kws_files_present` 移入 `src/model_library/asset.rs`
- Modify: `src/model_library/mod.rs:27`、`src/model_library/registry.rs:12`（`use crate::kws::model::` → `use crate::model_library::asset::`）
- Modify: `src/asr/mod.rs:22`、`src/asr/config.rs`、`src/asr/dictate.rs:8`、`src/tts/mod.rs:23`、`src/tts/config.rs`、`src/cli.rs:420,551`（同上替换）
- Delete: 暂不删 `src/kws/`（Task 2 处理）

**Steps:**
1. `cp` 语义迁移：新建 `src/model_library/asset.rs`，内容为 `kws/model.rs` 全文 + `kws_files_present` 函数（从 `kws/config.rs` 移入），修正模块 doc-comment
2. 在 `src/model_library/mod.rs` 加 `pub mod asset;`，全局替换 `crate::kws::model::` → `crate::model_library::asset::`、`crate::kws::config::kws_files_present` → `crate::model_library::asset::kws_files_present`
3. `cargo check` 直到零错误（此任务只改引用路径，不改逻辑）

**Verify:** `cargo test -p zapmomo model_library` 通过；`grep -rn "kws::model" src/ src-tauri/src/` 为空
**Commit:** `refactor(core): 迁移 kws/model.rs 模型下载基础设施至 model_library/asset`

---

### Task 2: 删除伴侣模块与 llm/dsh/live2d，清理 Cargo 依赖

**Files:**
- Delete: `src/kws/`（剩余：mod.rs/config.rs/token.rs/reaction.rs）、`src/speaker/`、`src/voice/`、`src/llm/`、`src/dsh/`、`src/live2d/`、`src/companion.rs`、`src/companion_share.rs`、`src/companion_welcome.rs`、`src/companion_sprites.rs`、`src/companion_store.rs` 及 `src/lib.rs` 中其余 `companion*` 模块声明
- Modify: `src/lib.rs:2-22`（模块声明表删 `companion*`/`dsh`/`kws`/`live2d`/`llm`/`speaker`/`voice`）
- Modify: `Cargo.toml`：删 `sherpa-onnx`（**暂留**，asr/tts 仍引用——本任务先删以下）、`async-openai`、`genai`、`pinyin`、`tiny_http`、`zip`、`ctrlc`、`futures-util`（若 Task 4/5 确认无残余消费者再删）
- Delete: `integrations/dsh-plugin/`（整个目录）、`.github/workflows/npm-publish.yml`

**Steps:**
1. `git rm -r` 上述目录；清 `src/lib.rs` 模块声明
2. `cargo check 2>&1 | head -50`，按报错清引用（预期报错点：`src/cli.rs` 的 Kws/Speaker/Voice/Llm 子命令、`src/config/settings.rs` 的 Kws/Llm/Voice/Speaker/Live2d/Dsh/Bubble 段、`src/config/shortcuts.rs`）
3. `src/config/settings.rs`：删 `KwsSettings`(322-368)、`Live2dSettings`(558-598)、`LlmSettings`(601-668)、`VoiceSettings`(671-724)、`DshSettings`(727-745)、`SpeakerSettings`(748-786)、`BubbleSettings`(802) 及 `AppConfig` 对应字段（251/260/263/266/269/278/284）、`get_companions_store_dir`(53)/`legacy_companions_dir`(60)
4. `src/config/shortcuts.rs`：删 KWS/LLM/会话/角色相关 `ShortcutAction`
5. Cargo.toml 删 `async-openai`/`genai`/`pinyin`/`tiny_http`/`zip`/`ctrlc`；`cargo check` 后若 `futures-util` 无引用一并删

**Verify:** `cargo clippy --workspace -- -D warnings` 全绿；`grep -rn "kws\|speaker\|live2d\|companion\|dsh" src/ --include="*.rs" -il` 仅剩允许文件（无）
**Commit:** `refactor(core): 移除伴侣能力模块（kws/speaker/voice/llm/dsh/live2d/companion）`

---

### Task 3: Resampler 去 sherpa 化（TDD）

**Files:**
- Modify: `src/audio.rs:134-149`（Resampler）、`:4`、`:277-288`（注释）
- Test: `src/audio.rs` 内 `#[cfg(test)]`

**Step 1: 写失败测试**

```rust
#[test]
fn resampler_identity_when_same_rate() {
    let mut r = Resampler::new(16_000, 16_000);
    let input: Vec<f32> = (0..160).map(|i| (i as f32 * 0.1).sin()).collect();
    assert_eq!(r.process(&input), input);
}

#[test]
fn resampler_scales_length_linearly() {
    let mut r = Resampler::new(16_000, 48_000);
    let input = vec![0.0_f32; 1600]; // 0.1s @16k → 0.1s @48k = 4800 样本
    assert_eq!(r.process(&input).len(), 4800);
}

#[test]
fn resampler_interpolates_midpoint() {
    let mut r = Resampler::new(2, 4); // 2Hz→4Hz：[0,1] → [0,0.5,1,1.5]
    assert!((r.process(&[0.0, 1.0])[1] - 0.5).abs() < 1e-6);
}
```

**Step 2:** `cargo test -p zapmomo resampler` → 预期 FAIL（现实现包装 sherpa，接口不同）

**Step 3: 实现**（手写线性插值，替换 `sherpa_onnx::LinearResampler` 包装；保留 `new(input_rate, output_rate)` / `process(&[f32]) -> Vec<f32>` 签名以最小化调用方改动；同采样率直接返回 clone；清 `:4`、`:277-288` 注释中的 sherpa 字样）

**Step 4:** `cargo test -p zapmomo resampler` → PASS
**Commit:** `refactor(audio): 手写线性插值 Resampler 替换 sherpa 依赖`

---

### Task 4: TTS 收窄为 Qwen3-TTS 单引擎

**Files:**
- Modify: `src/audiocpp/families.rs`（删 `OMNIVOICE`/`VOXCPM2` 及其测试断言；`request_options()` 的 voxcpm2 分支删）
- Modify: `src/tts/config.rs`（`TtsModelKind` 收窄为 `{ Qwen3Tts06, Qwen3Tts17 }`；族表/preflight 相应裁剪）
- Modify: `src/tts/mod.rs`（删 sherpa `TtsEngine` 分支 :15-17,274,336,345；保留 audiocpp 路径、`rodio` 播放、`install` :442-485）
- Modify: `src/model_library/registry.rs`（`registry_tts_kind` 只映射 qwen3 两族）

**Steps:**
1. families.rs 删两族常量 + `family_desc` 两个分支 + 测试改为只覆盖 qwen3 两尺寸
2. tts/config.rs 收窄枚举 → `cargo check` 报错点逐个清（预期：`src/cli.rs` 的 `--sid`/`--voice` 参数语义、前端无关）
3. tts/mod.rs 删 sherpa 分支（`use sherpa_onnx` 一并删）
4. registry.rs：`ModelType`/`TtsModelKind` 映射收窄
5. 注意 `families.rs` 的 `allows_named_voice`/`supports_streaming` 字段此刻仅剩 qwen3 值，保留字段（二期加族用扩展点）

**Verify:** `cargo test -p zapmomo tts audiocpp families` 全绿
**Commit:** `refactor(tts): TTS 引擎收窄为 Qwen3-TTS（0.6B/1.7B）`

---

### Task 5: ASR 改造——offline.rs 接 audiocpp（补齐转写缺口，TDD）

**Files:**
- Modify: `src/asr/offline.rs`（362 行：删 sherpa `OfflineRecognizer`，改走 `crate::audiocpp::client::AudiocppAsr`）
- Modify: `src/asr/mod.rs`（删 `AsrEngine` 流式引擎、`use sherpa_onnx`(:15)、`reaction.rs` 引用；保留 :260-298 install 函数与 `Resampler`/`start_capture` 引用）
- Delete: `src/asr/reaction.rs`（依赖 sherpa `RecognizerResult`）
- Test: `src/asr/offline.rs` 内 `#[cfg(test)]`

**Step 1: 写失败测试**（`transcribe_wav` 的请求构造部分抽为纯函数后测）：

```rust
#[test]
fn transcription_request_uses_qwen3_model_id() {
    let (model, language) = transcription_request_parts(&None);
    assert_eq!(model, "qwen3-asr-0.6b");
    assert_eq!(language, None, "语言自动识别，不显式传");
}
```

**Step 2:** `cargo test -p zapmomo transcription_request` → FAIL

**Step 3: 实现**：`transcribe_wav`（`asr/mod.rs:310`，现仅分发 sherpa）重写为：
读 wav bytes → `AudiocppAsr::transcribe`（`audiocpp/client.rs:281`，multipart wav + model + language）→ 返回文本。删 `src/asr/offline.rs` 的 sherpa 路径，保留文件读取/校验逻辑。CLI `asr transcribe` 与 Tauri `transcribe_audio` 因此自动获得 audiocpp 能力。

**Step 4:** `cargo test` PASS；标注「真机 E2E 冒烟」到验收任务
**Commit:** `feat(asr): 文件转写接入 audiocpp qwen3_asr 后端，移除 sherpa 离线识别`

---

### Task 6: dictate 简化——无 VAD 录音转写

**Files:**
- Modify: `src/asr/dictate.rs`（272 行：删 SileroVAD/`VadModel` 分段逻辑(:9)，保留 `crate::audio::start_capture`(:126)）
- Modify: `src/asr/mod.rs`（`AsrCmd::Dictate` 执行路径改为一站式：开始采集 → 停止信号 → 整段 wav → Task 5 的 audiocpp 转写 → 输出文本）

**Steps:**
1. 删 VAD 分段；`DictateState` 简化为「录制中/完成」两态
2. 停止时把 f32 样本经 `Resampler` 转 16kHz mono 写 wav → 调 `transcribe_wav`
3. CLI 语义不变：`audiofn asr dictate`，Ctrl-C/回车停止后输出转写文本

**Verify:** `cargo clippy --workspace -- -D warnings`；手动冒烟列入最终验收
**Commit:** `refactor(asr): dictate 改为免 VAD 录音转写（整段送 qwen3_asr）`

---

### Task 7: cli.rs 裁剪 + 删除 sherpa-onnx 依赖

**Files:**
- Modify: `src/cli.rs`：删 `Kws`/`Speaker`/`Voice`/`Llm` 子命令与 `greet` 命令（:42-67 命令枚举、:103-273 相关定义、`apply_backend_override` :886 保留）；帮助文本去掉 zapmomo 字样（:103,105,144,146,192,194,241,266,268）
- Modify: `Cargo.toml`：删 `sherpa-onnx`（此刻应无引用）、`rodio` **保留**（合成播放）
- Delete: `tests/remote_llm_smoke.rs`（整文件）
- Modify: `tests/integration_test.rs`：删 `test_cli_speaker_*` 两个用例与 `greet` 用例（:6,20,43,104）；保留 config/datetime/logging 用例，`use zapmomo::` 暂不动（品牌迁移在 Task 16）

**Steps:**
1. 删子命令与分发臂（`src/main.rs` 的 match 同步）
2. `cargo tree -i sherpa-onnx` 确认无消费者 → Cargo.toml 删依赖
3. `cargo test --workspace`

**Verify:** `cargo run -- --help` 只显示 asr/tts/config/completion；`cargo tree -i sherpa-onnx` 报「package not found」
**Commit:** `refactor(cli): 裁剪 CLI 至 asr/tts/config/completion，移除 sherpa-onnx 依赖`

---

### Task 8: src-tauri/lib.rs 裁剪（分四个子任务，每个独立 commit）

8941 行大文件，按域拆分，**顺序执行**，每子任务后 `cargo check -p zapmomo-app` 必须过。

**Task 8a: 删 dsh + LLM 域**
- Commands：`get_dsh_config`/`set_dsh_params`/`get_dsh_bridge_status`/`test_dsh_announce`/`detect_dsh_integration`/`install_dsh_plugin`/`uninstall_dsh_plugin`（:3336-3522）；`get_llm_config`/`load_llm_model`/`unload_llm_model`/`chat_llm`/`stop_llm`/`is_llm_ready`/`set_llm_connection`/`set_llm_params`/`set_llm_system_prompt`/`get_conversation_records`/`clear_conversation_records`（:2265-2376, :3766-3828）
- 线程/实现：`forward_llm_events`/`forward_sprite_events`（:2218-2263）、dsh 事件处理/`dsh_llm_worker`/`dsh_announcer`/`start_dsh_bridge_impl`（:2935-3321）
- 状态：`LlmState`/`DshBridgeState`/`DshInstallState`（:7428-7442 `.manage()`）；自动启动段 :7658-7668
- 测试：`llm_connection_tests`（:8844）
- Commit: `refactor(app): 移除 dsh 桥与 LLM 对话 commands`

**Task 8b: 删 KWS + 声纹域**
- KWS：`get_kws_config`(:407)、`start_listen`/`stop_listen`/`is_listening`(:1202-1242)、`download_kws_model`(:1250)、`set_kws_*`(:3892-3936)、`start_listen_impl`(:1080-1200)、`ListenState`/`DownloadState`、快捷键 KWS 动作
- 声纹：`get_speaker_config`(:572)、`set_speaker_*`(:598-613)、`download_speaker_model`(:636)、`record_speaker_sample`(:702)、`speaker_resume_mic`(:824)、`speaker_enroll`(:892)、`list_speakers`/`remove_speaker`(:957-972)、`speaker_identify_wav`(:1028)、`SpeakerState`、`speaker_params_tests`(:8203)
- Commit: `refactor(app): 移除 KWS 与声纹识别 commands`

**Task 8c: 删角色/Live2D/桌宠/chatbox/bubble 域**
- Commands：:4088, :4521-4879, :5626-5817, :6336, :6609, :7530（清单见设计文档 §5）+ `save_chatbox_position`(:4849) + `get_hide_dock_icon`/`set_hide_dock_icon`(:5727-5741) + `get_autostart`/`set_autostart`(:5811-5817)
- 实现：伙伴窗口构建/reconcile/migrate_legacy/register_motions（:7742-7820）、chatbox/bubble 窗口构建（:7864-8009）、`on_window_event` 几何联动（:8102-8164）、sprite 转发线程（:7591-7596）、`#[cfg(windows)] apply_companion_layer_platform`(:5355)
- 配置残留：`ChatboxSettings`/`CompanionWindowPosition` 等（settings.rs 已在 Task 2 删，此处清残余引用）
- Commit: `refactor(app): 移除角色窗口/Live2D/chatbox/bubble commands 与窗口`

**Task 8d: 删 voice 会话域 + invoke_handler 收尾**
- `start_voice_session`/`stop_voice_session`/`is_voice_session_running`/`get_voice_enabled`/`set_voice_enabled`/`send_voice_text`（:2736-2817）、`make_voice_emit`/`start_voice_session_impl`（:2389-2539）、`VoiceSessionState`、voice 自动启动（:7598-7656）
- `invoke_handler`（:7443-7571）只保留存活命令；`preflight_tests`(:8749) 裁剪为 qwen3 场景
- **保留段（勿动）**：audiocpp 环境注入 :7670-7719、`RunEvent::Exit` 兜底 :8165-8173、ASR/TTS/模型库/存储/快捷键 commands
- Commit: `refactor(app): 移除语音会话，invoke_handler 收敛至 ASR/TTS/模型库`

---

# 阶段 2：引擎白名单 + 模型资产收窄

### Task 9: 模型清单裁剪

**Files:**
- Modify: `models/manifest.json`：只留 3 项资产——`qwen3-tts-06b-base-audiocpp`、`qwen3-tts-17b-base-audiocpp`、`qwen3-asr-0.6b-audiocpp`
- Modify: `models/model_registry.json`、`models/verified_registry.json`：删 kws/asr(sherpa)/llm/tts(sherpa/omnivoice/voxcpm2)/speaker 条目
- Modify: `src/model_library/registry.rs`：`ModelType` 删 `Kws`/`Llm`/`Speaker` 变体；`required_files_for_role` 删对应 role 分支；`runtime="llama.cpp"` 卡片字符串清理（:652,906,1881）

**Steps:**
1. 裁剪三个 JSON → `cargo check` → 按 `serde` 报错/`registry.rs` 编译错误收窄枚举
2. `cargo test -p zapmomo model_library`

**Verify:** `cargo run -- config` 正常；模型库枚举仅 asr/tts
**Commit:** `chore(models): 模型资产收敛为 qwen3_tts/qwen3_asr 三项`

---

### Task 10: 构建白名单收窄 + 移除 Windows

**Files:**
- Modify: `.github/workflows/release.yml`：`AUDIOCPP_MODELS=omnivoice,voxcpm2,qwen3_tts,qwen3_asr` → `qwen3_tts,qwen3_asr`（:176-178）；删 Windows 矩阵腿（:56-62）、Windows sherpa cache（:121,128-144）、`Fetch audio.cpp prebuilt (Windows)`（:211-266）、产物名 Windows 行（:366-367）、release body Windows 行（:390）、`files: *.msi/*.exe`（:425-432）；dmg 卷名注释 `ZapMomo`→`AudioFn`（:305）
- Modify: `scripts/fetch-audiocpp-dev.sh`：白名单同步（:81-84）；删 Windows/MINGW 分支与 CUDA DLL 收集段（:26-31 等）；更新头部注释（"裁剪"措辞改为"只编译两族"）
- Delete: `src-tauri/tauri.windows.conf.json`
- Modify: `src-tauri/Cargo.toml`：删 `[target.'cfg(windows)'.dependencies] windows = "0.61"`（:45-52）
- Modify: `.cargo/config.toml`：重写（清 llama.cpp/sherpa 遗留注释；Windows `/MT`/`+crt-static` 段随平台收敛删除）
- Modify: `.github/workflows/ci.yml`：删 windows 腿（:149、:215）、sherpa cache 段（:43-64、:105-120、:174-189、:232-247）、Windows 后缀分支（:201）
- Modify: `src-tauri/src/lib.rs`：`strip_extended_prefix`（:7674-7719 中 Windows `\\?\` 分支）加 `#[cfg(windows)]` 门控或删除

**Steps:**
1. 逐文件修改；release.yml/ci.yml 用 YAML lint 心算校验（或 actionlint，若无则跳过）
2. `bash -n scripts/fetch-audiocpp-dev.sh` 语法检查
3. `cargo check --workspace`

**Verify:** `grep -n "windows" .github/workflows/release.yml .github/workflows/ci.yml src-tauri/Cargo.toml` 无残留；`grep -n "AUDIOCPP_MODELS" -A0` 显示两族
**Commit:** `chore(build): 引擎白名单收窄为 qwen3 两族，平台收敛 macOS/Linux`

---

### Task 11: 脚本与杂项清理

**Files:**
- Delete: `scripts/download-kws-model.sh`、`scripts/run-kws-model-tests.sh`
- Modify: `models/THIRD_PARTY_NOTICES.md`：重写为 qwen3-tts/qwen3-asr/audio.cpp/sherpa 残留（如有）实际清单，修正漏列 qwen3_asr 的漂移（:96）
- Delete: 根目录 `COMPANION_VOICE_UPLOAD_DESIGN.md`、`COMPANION_WAKE_WORD_WELCOME_DESIGN.md`、`COMPANION_PACK_SHARE_DESIGN.md`（其中音色上传设计的有用约束已在 Task 5/3 的实现注释中体现；如需保留线索移入 `docs/plans/` 归档目录）

**Verify:** `ls scripts/`；`grep -rn "sherpa" models/THIRD_PARTY_NOTICES.md` 与实际依赖一致
**Commit:** `chore: 清理 kws 脚本、伴侣设计文档，重写第三方声明`

---

# 阶段 3：前端裁剪

### Task 12: HTML 入口与 Vite 多页配置

**Files:**
- Delete: `src-tauri/frontend/companion.html`、`bubble.html`、`chatbox.html`、根级 `src/companion.tsx`、`src/bubble.tsx`、`src/chatbox.tsx`
- Modify: `src-tauri/frontend/vite.config.ts:23-28`（多页入口只留 `settings.html`）

**Verify:** `pnpm --dir src-tauri/frontend build` 成功
**Commit:** `refactor(frontend): 移除桌宠/气泡/输入条窗口入口`

### Task 13: 路由与页面裁剪

**Files:**
- Delete: `pages/ChatPage.tsx`、`CompanionPage.tsx`、`IntegrationsPage.tsx`、`pages/models/KwsPage.tsx`、`LlmPage.tsx`、`SpeakerPage.tsx` 及对应 `components/{kws,llm,speaker,integrations,companion*,live2d,gif,performance,bubble,chatbox}/`、`components/ListeningStatusBadge.tsx`、`components/home/overviewMeta.ts`（重写为 asr/tts 两卡）、`components/models/KwsModelSwitchMenu*`
- Modify: `App.tsx:28-43`（路由只留 `/home`、`/models`、`/models/asr`、`/models/tts`、`/settings`）、`components/layout/Sidebar.tsx:6-11`（导航项）、`pages/HomePage.tsx`（删 CompanionCard）
- Modify: `components/settings/`（删 `CompanionWindowSection.tsx`）

**Verify:** `pnpm --dir src-tauri/frontend build` + `pnpm --dir src-tauri/frontend test` 成功
**Commit:** `refactor(frontend): 路由收敛为模型库/ASR/TTS/设置四组页面`

### Task 14: hooks/lib/types 清理与依赖裁剪

**Files:**
- Delete hooks: `useKwsConfig`、`useKwsModelSwitch`、`useListening`、`useLive2dConfig`、`useLlm`、`useResults`、`useSpeakerConfig`、`useSpeakerModelDownload`、`useSpeakers`、`useCompanionLibrary`、`useVoiceSession`
- Delete lib: `companionFormat`、`companionHitRegion`、`dshIntegration*`、`dshMotion`
- Modify: `types/modelLibrary.ts`、`types/tauri.ts`（删 KWS/LLM/Speaker 类型）
- Modify: `src-tauri/frontend/package.json`：删 `pixi-live2d-display`、`pixi.js`、`react-intersection-observer`；`name` → `audiofn-frontend`（品牌统一在 Task 16 亦可）
- Modify: `hooks/useAsrListening*` 相关调用点（ASR 实时监听 commands 已在 Task 8 删除，`start_asr_listen` 前端调用改为 dictate 面板承载）

**Steps:**
1. `pnpm install` 刷新 lockfile；`pnpm --dir src-tauri/frontend build && test && lint`
2. `grep -rn "invoke(\"get_kws\|chat_llm\|speaker_" src-tauri/frontend/src` 为空

**Verify:** 前端三件套（build/test/lint）全绿
**Commit:** `refactor(frontend): 清理伴侣功能 hooks/类型与 pixi 依赖`

---

# 阶段 4：品牌迁移 + 文档/发布

### Task 15: Cargo/package/Tauri 品牌改名

**Files:**
- Modify: `Cargo.toml`（:11 description、:15 `name = "audiofn"`、:26 bin `audiofn-cli`、:22-24 注释）、`Cargo.lock`（由 cargo 自动刷新）、`src-tauri/Cargo.toml`（:2 `audiofn-app`、:11 bin `AudioFn`）、`package.json`、`docs/package.json`、`typos.toml:14`
- Modify: `src-tauri/tauri.conf.json`：`productName: "AudioFn"`、`identifier: "com.audiofn.audiofn"`、externalBin 路径不变
- Modify: 全部 `use zapmomo::` → `use audiofn::`（`src-tauri/src/lib.rs:5838` 等、`tests/integration_test.rs:3`、`src/main.rs`）
- Modify: `release-plz.toml`

**Steps:**
1. `grep -rl "zapmomo" --include="*.toml" --include="*.json" . | grep -v node_modules | grep -v target` 逐个改
2. `cargo build` 重新生成 lock；`pnpm install`

**Verify:** `cargo clippy --workspace -- -D warnings && cargo test` 全绿
**Commit:** `chore: 品牌迁移 zapmomo → audiofn（crate/bin/包名/Tauri 标识）`

### Task 16: Rust 代码品牌字符串与数据目录

**Files:**
- Modify: `src/config/settings.rs:12`：`const PROJECT_DIR: &str = ".audiofn"`（唯一定义点）
- Modify: `src/logging.rs:61`：`".zapmomo/logs"` → `".audiofn/logs"`（及该文件测试 :74-246 的硬编码）
- Modify: `src/model_library/mod.rs:940`、`install.rs:174`：`.zapmomo-lib.json` → `.audiofn-lib.json`
- Modify: `src/model_library/storage.rs:269`：`.zapmomo-probe` → `.audiofn-probe`（及该文件测试硬编码）
- Modify: `src/cli.rs` 帮助文本残留、`src/audiocpp/families.rs` 的 `registry_hint`（"zapmomo tts install-model" → "audiofn tts install-model"，:85,102,120,135）
- Modify: `src/asr/config.rs`、`src/tts/config.rs` 测试硬编码路径
- 决策：**不做** zapmomo → audiofn 存量数据迁移（新产品，YAGNI；`legacy_models_dir` 模式仅服务旧版 zapmomo 自迁移，可删）

**Verify:** `grep -rn "zapmomo" src/ src-tauri/src/ --include="*.rs" -i` 仅剩历史 changelog/注释类（应为零）；全量测试绿
**Commit:** `chore(core): 数据目录与用户可见文案迁移至 audiofn`

### Task 17: CI 收尾

**Files:**
- Modify: `.github/workflows/upload-baidu-pan.yml`（:5-7,34,47 网盘路径/品牌）
- Modify: `.github/workflows/docs.yml`（如有品牌引用）
- Delete: `.github/workflows/publish.yml` 中 sherpa cache（:33,36-51）
- 确认 `release.yml` 产物名：`AudioFn_macOS_arm64.dmg` / `AudioFn_Linux_amd64.{deb,rpm,AppImage}`

**Verify:** `grep -rni "zapmomo\|sherpa\|windows" .github/workflows/` 为零（或仅注释）
**Commit:** `chore(ci): 工作流品牌与平台收敛`

### Task 18: README / CONTRIBUTING / AGENTS 重写

**Files:**
- Rewrite: `README.md`、`README.en.md`（定位：本地 ASR+TTS 工具；特性：Qwen3-TTS-0.6B 音色克隆、Qwen3-ASR 转写、CLI + 桌面；快速命令对照 `audiofn asr/tts`；下载表只留 macOS 两架构 + Linux deb/rpm/AppImage）
- Rewrite: `CONTRIBUTING.md`（发布流程段落保留 release-plz + tauri-action 结构，品牌替换）
- Rewrite: `AGENTS.md`、`CLAUDE.md`（项目概述/结构/命令全部对齐裁剪后现状）
- Modify: `CHANGELOG.md`、`src-tauri/CHANGELOG.md`（新起点，标注 audiofn 0.1.0）

**Verify:** `grep -rni "zapmomo\|companion\|live2d\|唤醒\|声纹" README.md CLAUDE.md AGENTS.md` 为零
**Commit:** `docs: 重写 README 与贡献文档为 audiofn 定位`

### Task 19: 最终验收（对照设计 §8）

1. `cargo fmt --check && cargo clippy --workspace -- -D warnings && cargo test` 全绿
2. `pnpm --dir src-tauri/frontend build && test && lint` 全绿
3. `scripts/fetch-audiocpp-dev.sh` 重新放置 sidecar（两族构建）→ `pnpm tauri dev` 可启动
4. CLI 端到端（真机）：`cargo run -- tts install-model --registry-id tts-qwen3-06b-base-q8-audiocpp` → `cargo run -- tts run --text "你好，audiofn" --reference-wav <样本> --reference-text <文本> --output /tmp/out.wav` → 播放验证音色
5. `cargo run -- asr install-model ...` → `cargo run -- asr transcribe /tmp/out.wav` 输出「你好，audiofn」
6. GUI 端到端：音色库录制参考音频 → 合成播放；转写页选 wav → 出文本
7. Linux 真机基准（如有）：记录 RTF 到 README「已知限制」

**Commit:** `chore: 一期验收通过`（如有修复随验收提交）

---

## 执行方式

推荐 **Subagent-Driven**（superpowers:subagent-driven-development）：每任务派发独立子代理执行，任务间我做代码评审——本工程 80% 是精确删除，子代理+评审能有效防误删共享代码（如 Task 1 的迁移陷阱）。
