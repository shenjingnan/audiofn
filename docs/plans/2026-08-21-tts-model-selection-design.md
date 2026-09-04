# TTS「选择模型」弹窗 —— 技术方案（第一期：铺管线）

日期：2026-08-21
参考实现：PR #134（ASR 选择模型弹窗，commit `b4c3bed1`）、PR #125（KWS 多模型切换，commit `a3134f2f`）

## 1. 背景与现状

「模型与能力」页中 KWS / ASR / LLM 均已具备「选择模型」弹窗（三者是近乎镜像的独立组件，
数据统一走模型库 `list_model_library`，保存走 `set_current_model`）。TTS 尚未接入该流程。

TTS 的能力底座已完整存在：

- 引擎层：`src/tts/mod.rs` `TtsEngine` 封装 sherpa-onnx `OfflineTts`（ZipVoice 中英双语 +
  Vocos 声码器，零样本音色克隆）。
- Tauri 命令：13 个 `tts_*` 命令已注册（`src-tauri/src/lib.rs`）。
- 模型库底座：`ModelType::Tts` 已是一等公民——`set_selected_model` 的 tts 分支存在、
  `install_managed_model` 支持 TTS 双资产（主包 + 声码器）下载校验、`delete_model` 天然覆盖。
- registry：`models/model_registry.json` 已注册 1 个 TTS 模型 `tts-zipvoice-distill-int8`
  （163MB，`required_assets=["tts","tts-vocoder"]`）。
- 前端：`/models/tts` 配置页完整（基础配置 / 测试语音 / 音色管理），但「下载模型」是
  legacy 一键下载（`download_tts_model`），未接入模型库弹窗。

**关键事实（已验证）：**

1. **切换链路已可用**：`set_current_model`（lib.rs:3507）对非 LLM 类型统一写 selection；
   TTS 每次合成现场 `TtsEngine::new`（lib.rs:907），Tauri 层不常驻引擎，因此「写配置 →
   下次合成生效」语义天然成立，无需 LLM 式热切换事务。
2. **目录互通**：legacy `download_tts_model` 与 registry managed 安装落同一目录
   （`get_models_dir()/sherpa-onnx-zipvoice-distill-int8-zh-en-emilia`，registry `name` 与
   manifest asset `name` 一致）——老用户已下载的模型在弹窗中会正确显示「已安装」，无迁移成本。
3. **差距仅三处**：
   - 后端：`RuntimeActuals`（`src/model_library/mod.rs:181`）无 tts 字段，
     `enrich_runtime_status`（:445）中 `ModelType::Tts => (None, false)`，运行状态恒 Inactive。
   - 后端：`set_selected_model`（:362）tts 分支只写 `model_dir`，未重置 vocoder/tokens 等
     文件级覆盖（ASR PR #134 的模式是切换时重置）。
   - 前端：无 `TtsModelDialog` / `TtsModelSwitchMenu` / `useTtsModelSwitch` 三件套，
     `ModelSummary.tsx` TTS 行是纯文本行（:251-273）。

## 2. 目标与非目标

### 目标（第一期）

- 「模型与能力」概览页 TTS 行与 KWS/ASR/LLM 对齐：模型名 + 「选择模型」快捷入口。
- TTS 配置页「下载模型」按钮**直接替换**为「选择模型」，统一走模型库弹窗（下载 / 设为当前 /
  卸载 / 进度）。
- 弹窗内 TTS 当前模型显示真实运行状态（空闲 / 合成中）。
- 语音会话运行中切换 TTS：**静默生效**（下次语音会话/下次合成用新模型，不打断、不提示）。

### 非目标（留第二期）

- 扩充 TTS 模型阵容（matcha / vits 等固定音色小模型）：需 `TtsEngine` 支持
  `OfflineTtsConfig` 其他模型分支（音色体系也不同：固定 speaker_id vs ZipVoice 参考音频克隆），
  另立方案。
- HF 在线目录为 TTS 检索/下载新模型。

### 已确认的决策

| 决策点 | 结论 |
| --- | --- |
| 模型阵容 | 分两期，第一期只接现有 ZipVoice 单模型铺管线 |
| 配置页「下载模型」按钮 | 直接替换为「选择模型」（TTS 无 ASR 式 legacy 分流必要：目录/内容相同，双资产均 required） |
| 语音会话中切换 | 静默生效（现状通用分支行为即为静默，无需新增会话分支） |

## 3. 技术方案

### 3.1 后端（2 处改动）

**A1. 运行时状态注入**

- `src/model_library/mod.rs`：
  - `RuntimeActuals<'a>` 增加 `pub tts: Option<&'a Path>` 与 `pub tts_active: bool` 字段。
  - `enrich_runtime_status` 中 `ModelType::Tts => (a.tts, a.tts_active)`。
- `src-tauri/src/lib.rs` `list_model_library`（:3376）构造 `RuntimeActuals` 处：
  - `tts`：`settings.tts` 解析后的 `model_dir`（与 KWS/ASR 注入方式一致，从 settings 读）。
  - `tts_active`：`TtsState` 合成线程存活（复用 `is_tts_synthesizing` 的判定）。
- 效果：当前 TTS 模型空闲 → `Inactive`、合成中 → `Active`；非当前模型恒 `Inactive`
  （`enrich_runtime_status` 已有 `!m.current` 短路）。

**A2. 切换时重置文件级覆盖**

- `set_selected_model`（`src/model_library/mod.rs:362`）tts 分支：当目标 `model_dir` 与当前
  不同时，重置 `vocoder` / `tokens` / `lexicon` / `data_dir` / `encoder` / `decoder` 等
  文件级覆盖字段为 None（对齐 ASR 分支重置 encoder/decoder/joiner/tokens 的模式）。
  第一期单模型下无实际影响，防止手工改过配置的用户切模型后残留旧覆盖，亦为第二期铺路。

**无需改动**：`set_current_model` TTS 走现有通用分支（静默生效 + `effective_immediately: true`）；
`download_library_model` / `delete_model` / `install_managed_model` 对 TTS 已天然支持。

### 3.2 前端（镜像 ASR 三件套）

| # | 文件（`src-tauri/frontend/src/`） | 内容 |
| --- | --- | --- |
| B1 | `components/tts/ttsMeta.ts`（新） | `TTS_PRESETS`（暂 1 项 `tts-zipvoice-distill-int8`：显示名/语言/大小/描述）+ `isDefaultTtsModelDir` / `modelNameFromDir` |
| B2 | `hooks/useTtsModelSwitch.ts`（新） | 镜像 `useAsrModelSwitch`：mount 时 `api.listModelLibrary()` 过滤 tts；`download_library_model` + `model-library-download-progress` 进度；`set_current_model` 设当前；`delete_model` 卸载。**差异**：无 stop→start 自动重启逻辑（TTS 无监听概念），`runtimeAction` 仅用于刷新配置 |
| B3 | `components/tts/TtsModelDialog.tsx`（新） | 镜像 `AsrModelDialog`：`LibraryDialog` 外壳 + 列表（未安装→下载 / 已安装→设为当前+卸载 / 当前→绿色标记）+ `Progress` 进度条（`useSmoothProgress`）+ `ModelConfirmDialog` 卸载确认 |
| B4 | `components/models/TtsModelSwitchMenu.tsx`（新） | 镜像 `AsrModelSwitchMenu`：概览行下拉入口（当前模型名 + 选择模型按钮） |
| B5 | 接入点 3 处 | ① `ModelSummary.tsx` TTS 行（:251-273）从纯文本改为模型名 + `TtsModelSwitchMenu`；② `pages/models/TtsPage.tsx` 持有弹窗 open 状态；③ `components/tts/TtsBasicConfig.tsx` 「下载模型」按钮替换为「选择模型」（`onOpenModelDialog`），legacy `download_tts_model` 调用与 `tts-model-download-progress` 监听可从 `useTts.ts` 移除（弹窗走模型库通道） |

注意：`useModelLibrary.ts`（模型库页全局动作）动作后已同步刷新 kws/asr/llm/tts 四个配置，
TTS 弹窗与模型库页状态互通无需额外处理。

### 3.3 数据流（与 ASR 一致）

```
TtsModelDialog / TtsModelSwitchMenu
  → useTtsModelSwitch
    → api.listModelLibrary()            # 列表 + installState + current + runtimeStatus
    → api.downloadLibraryModel(id)      # 事件 model-library-download-progress
    → api.setCurrentModel(id)           # 写 [tts].model_dir，立即生效（下次合成用新模型）
    → api.deleteModel(id)               # 卸载（当前/下载中/合成中会被后端拒绝）
```

## 4. 边界与兼容性

- **老用户**：曾用「下载模型」按钮（legacy）安装过 → 同一目录，弹窗显示「已安装」，可直接设为当前。
- **卸载保护**：`delete_model` 已拒绝删除「当前/下载中」模型；TTS 无常驻引擎，无 LLM 式运行中拒绝。
- **合成中切换**：在飞合成线程持有旧 `cfg` 克隆，自然完成；下一次合成读取新配置。静默生效。
- **配置页联动**：设为当前后刷新 `get_tts_config`，`TtsBasicConfig` 当前模型名/路径随之更新；
  未配置模型时保留现有「未配置」引导（改为引导打开「选择模型」弹窗）。
- **移除 legacy 下载通道**（`useTts.ts` 中 `download_tts_model` 相关）需确认无其他引用
  （`ModelSummary` 未配置态的「下载」入口同步改为打开弹窗）。

## 5. 测试计划

### Rust

- `model_library` 单测：`enrich_runtime_status` TTS 状态矩阵（镜像 `test_runtime_status_matrix`，
  mod.rs:1285）：未配置/已配置空闲/合成中/非当前等用例。
- `set_selected_model` TTS 分支：切换后 `model_dir` 更新且文件级覆盖被重置的断言。

### 前端（Vitest）

- `useTtsModelSwitch`：列表加载 / 下载进度 / 设为当前 / 卸载分支（镜像 ASR hook 测试，
  注意 Vitest 4 mock 坑：构造器 mock 用 function 实现）。
- `TtsModelDialog` / `TtsModelSwitchMenu`：三态渲染与回调（镜像 ASR 组件测试）。

### 验收清单

1. 概览页 TTS 行出现「选择模型」入口，弹窗列出 ZipVoice（老用户显示已安装）。
2. 未安装 → 弹窗下载（进度条平滑）→ 完成变「已安装」。
3. 设为当前 → 配置页当前模型名变化 → 「测试语音」正常出声。
4. 合成进行中切换模型：在飞合成完成，下一次合成用新模型，无报错无提示。
5. 模型库页 TTS 卡片「设为当前/卸载」与 TTS 配置页状态互通。
6. 卸载当前模型被拒绝；卸载非当前模型后配置页回退「未配置」引导。
7. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` 通过；
   `tsc -b` 与前端 vitest 通过（注意根目录 `tsc --noEmit` 空通过问题，用 `tsc -b`）。

## 6. 第二期展望（不在本期实施）

- registry/manifest 新增 1-2 个轻量固定音色模型（如 matcha-icefall-zh-baker、vits-melo-tts-zh_en）。
- `TtsEngine` 扩展 `OfflineTtsConfig` 的 vits/matcha/kokoro 分支；音色系统区分
  「参考音频克隆（ZipVoice）」与「固定 speaker_id」两种模式；`TtsVoicesDialog` 按模型类型分流。
- `tts/config.rs` 增加探测式 `detect_default_*`（对齐 ASR PR #134 的文件名探测）。
