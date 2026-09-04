# 全局快捷键自定义 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 设置页新增「快捷键」区块，允许用户为 4 个高频操作（显示/隐藏桌宠、语音会话开关、打断播报、打开设置）自定义系统级全局快捷键；默认不注册任何快捷键。

**Architecture:** 配置存根 crate `[shortcuts]` 分节（`src/config/shortcuts.rs` 纯逻辑 + 校验）；tauri 侧引入 `tauri-plugin-global-shortcut`，`set_shortcut` command 遵循「先注册成功、再落盘」；触发经 `dispatch_shortcut` 复用现有内部函数，打断播报通过置位 voice 会话既有的 `barge_in` 标志实现。前端新增 `ShortcutsSection` 组件（录制态捕获 keydown）。

**Tech Stack:** Rust（根 crate `zapmomo` + `src-tauri/`）、tauri-plugin-global-shortcut 2、React 19 + Vitest 4 + Testing Library。

**设计文档:** `docs/plans/2026-08-20-global-shortcuts-design.md`

**对设计文档的一处修正:** capabilities 无需改动——快捷键全部在 Rust 侧注册（`on_shortcut` + 闭包），前端只调用自定义 command（`set_shortcut` 等），不触碰插件的前端 API，因此不需要 `global-shortcut:allow-*` 权限。

**关键现状锚点（写码前速览）:**

- `src/config/mod.rs` 目前只有 `pub mod settings;`
- `AppConfig` 定义在 `src/config/settings.rs:203-245`，末尾字段是 `model_library`（244 行）
- `VoiceSession.barge_in: Arc<AtomicBool>` 在 `src/voice/session.rs:80`；主循环 `run()`（session.rs:215-238）每轮检查，`Thinking|Speaking` 时置位即触发 `do_barge_in()` 回 Armed
- `VoiceSessionState` 在 `src-tauri/src/lib.rs:113-129`；`start_voice_session_impl` 在 lib.rs:1580，会话线程在 lib.rs:1627-1653 创建并运行 session
- `stop_tts` command 在 lib.rs:1078；`stop_voice_session_inner` 在 lib.rs:1702；`toggle_companion_window` 在 lib.rs:2523；`show_settings_window` 在 lib.rs:2515
- command 注册表 `generate_handler!` 在 lib.rs:3668-3762；`setup` 闭包在 lib.rs:3763 起
- 前端 api 封装 `src-tauri/frontend/src/lib/tauri.ts:61` 起；`getHideDockIcon` 在 186-187 行附近
- 设置页 Section 的 JSX 风格见 `SettingsPage.tsx:232-296`（`<section className="overflow-hidden rounded-[16px] border border-panel-border bg-panel-background">` + 图标标题 + `<dl className="divide-y divide-divider">`）
- 测试 mock 模式见 `SettingsPage.test.tsx:8-74`（`vi.hoisted` + `vi.mock("@tauri-apps/api/core")` + `invokeMock.mockImplementation` 按 command 名分发）

---

### Task 1: 根 crate shortcuts 配置模块（TDD）

**Files:**
- Create: `src/config/shortcuts.rs`
- Modify: `src/config/mod.rs`（加一行 `pub mod shortcuts;`）
- Modify: `src/config/settings.rs`（`AppConfig` 加 `shortcuts` 字段，244 行 `model_library` 之后）

**Step 1: 写失败测试**

创建 `src/config/shortcuts.rs`，先只写类型骨架 + 完整测试模块：

```rust
//! 全局快捷键配置：`[shortcuts]` 分节与 action 定义。
//!
//! accelerator 为 tauri-plugin-global-shortcut 标准格式（修饰键 + 主键，`+` 分隔，
//! 如 `CmdOrCtrl+Shift+Z`）。所有字段缺省 `None` = 不注册任何全局快捷键（默认策略，
//! 老用户升级零变化）；注册/解绑在 tauri 侧完成，本模块只管配置与纯逻辑校验。

use serde::{Deserialize, Serialize};

/// 可绑定全局快捷键的操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShortcutAction {
    /// 显示/隐藏桌宠窗口
    ToggleCompanion,
    /// 语音会话 开/关
    ToggleVoiceSession,
    /// 打断当前回复（停止生成与朗读，回到待唤醒）
    InterruptReply,
    /// 打开设置窗口
    OpenSettings,
}

/// 快捷键配置分节（action → accelerator；`None` = 未绑定）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ShortcutsSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toggle_companion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toggle_voice_session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupt_reply: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_settings: Option<String>,
}

/// 插件可识别的修饰键（结构校验用；最终合法性由插件注册裁决）。
const MODIFIERS: &[&str] = &[
    "CmdOrCtrl",
    "CommandOrControl",
    "Cmd",
    "Command",
    "Ctrl",
    "Control",
    "Alt",
    "Option",
    "Shift",
    "Super",
    "Meta",
];

/// 校验 accelerator：非空、至少一个修饰键 + 一个主键（拒绝裸键与纯修饰键组合）。
pub fn validate_accelerator(accelerator: &str) -> Result<(), String> {
    todo!("Task 1 Step 3 实现")
}
```

测试模块（同文件底部）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_ident_roundtrip() {
        for action in ShortcutAction::ALL {
            assert_eq!(ShortcutAction::from_ident(action.as_str()), Some(action));
        }
        assert_eq!(ShortcutAction::from_ident("nope"), None);
    }

    #[test]
    fn test_validate_ok() {
        assert!(validate_accelerator("CmdOrCtrl+Shift+Z").is_ok());
        assert!(validate_accelerator("CmdOrCtrl+Shift+,").is_ok());
        assert!(validate_accelerator("Alt+F4").is_ok());
    }

    #[test]
    fn test_validate_rejects() {
        assert!(validate_accelerator("").is_err()); // 空
        assert!(validate_accelerator("Z").is_err()); // 裸键
        assert!(validate_accelerator("Shift").is_err()); // 纯修饰键
        assert!(validate_accelerator("Foo+Z").is_err()); // 未知前缀段
        assert!(validate_accelerator("CmdOrCtrl+Shift++").is_err()); // 空段
    }

    #[test]
    fn test_settings_get_set_clear() {
        let mut s = ShortcutsSettings::default();
        assert_eq!(s.get(ShortcutAction::ToggleCompanion), None);
        s.set(ShortcutAction::ToggleCompanion, Some("CmdOrCtrl+Shift+Z".into()));
        assert_eq!(
            s.get(ShortcutAction::ToggleCompanion),
            Some("CmdOrCtrl+Shift+Z")
        );
        s.set(ShortcutAction::ToggleCompanion, None);
        assert_eq!(s.get(ShortcutAction::ToggleCompanion), None);
    }

    #[test]
    fn test_find_conflict() {
        let mut s = ShortcutsSettings::default();
        s.set(ShortcutAction::OpenSettings, Some("CmdOrCtrl+Shift+O".into()));
        // 同键异 action → 命中冲突
        assert_eq!(
            s.find_conflict(ShortcutAction::ToggleCompanion, "CmdOrCtrl+Shift+O"),
            Some(ShortcutAction::OpenSettings)
        );
        // 同 action 自身 → 不算冲突
        assert_eq!(
            s.find_conflict(ShortcutAction::OpenSettings, "CmdOrCtrl+Shift+O"),
            None
        );
        // 不同键 → 无冲突
        assert_eq!(
            s.find_conflict(ShortcutAction::ToggleCompanion, "CmdOrCtrl+Shift+P"),
            None
        );
    }

    #[test]
    fn test_toml_roundtrip_and_default_absent() {
        // 含 [shortcuts] 的配置可解析；未写分节时为 None（老配置兼容）
        let with_section = r#"
[shortcuts]
toggle_companion = "CmdOrCtrl+Shift+Z"
"#;
        let cfg: crate::config::settings::AppConfig = toml::from_str(with_section).unwrap();
        assert_eq!(
            cfg.shortcuts.unwrap().toggle_companion.as_deref(),
            Some("CmdOrCtrl+Shift+Z")
        );

        let empty: crate::config::settings::AppConfig = toml::from_str("").unwrap();
        assert!(empty.shortcuts.is_none());

        // 序列化：未绑定字段不落盘
        let mut s = ShortcutsSettings::default();
        s.set(ShortcutAction::InterruptReply, Some("CmdOrCtrl+Shift+X".into()));
        let out = toml::to_string(&s).unwrap();
        assert!(out.contains("interrupt_reply"));
        assert!(!out.contains("toggle_companion"));
    }
}
```

**Step 2: 跑测试确认失败**

Run: `cargo test -p zapmomo config::shortcuts 2>&1 | tail -5`
Expected: FAIL（`validate_accelerator` todo! panic / `ALL`、`from_ident`、`get`、`set`、`find_conflict` 未定义导致编译错误——先补齐方法签名使编译通过、测试断言失败也可，重点是红）

**Step 3: 实现完整逻辑**

替换 `todo!`，并给 `ShortcutAction` / `ShortcutsSettings` 补方法（放在 `ShortcutAction` impl 与 `ShortcutsSettings` impl 中）：

```rust
impl ShortcutAction {
    /// 全部可绑定操作（配置遍历 / 启动注册用）。
    pub const ALL: [ShortcutAction; 4] = [
        ShortcutAction::ToggleCompanion,
        ShortcutAction::ToggleVoiceSession,
        ShortcutAction::InterruptReply,
        ShortcutAction::OpenSettings,
    ];

    /// snake_case 标识：配置字段名 / 前端 command 参数。
    pub fn as_str(self) -> &'static str {
        match self {
            ShortcutAction::ToggleCompanion => "toggle_companion",
            ShortcutAction::ToggleVoiceSession => "toggle_voice_session",
            ShortcutAction::InterruptReply => "interrupt_reply",
            ShortcutAction::OpenSettings => "open_settings",
        }
    }

    /// 中文标签（错误文案「已绑定到 XX」用，与设置页展示一致）。
    pub fn label(self) -> &'static str {
        match self {
            ShortcutAction::ToggleCompanion => "显示/隐藏桌宠",
            ShortcutAction::ToggleVoiceSession => "语音会话 开/关",
            ShortcutAction::InterruptReply => "打断播报",
            ShortcutAction::OpenSettings => "打开设置",
        }
    }

    /// 从标识解析（前端 command 参数 → 枚举）。
    pub fn from_ident(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|a| a.as_str() == s)
    }
}

impl ShortcutsSettings {
    pub fn get(&self, action: ShortcutAction) -> Option<&str> {
        match action {
            ShortcutAction::ToggleCompanion => self.toggle_companion.as_deref(),
            ShortcutAction::ToggleVoiceSession => self.toggle_voice_session.as_deref(),
            ShortcutAction::InterruptReply => self.interrupt_reply.as_deref(),
            ShortcutAction::OpenSettings => self.open_settings.as_deref(),
        }
    }

    pub fn set(&mut self, action: ShortcutAction, accelerator: Option<String>) {
        let slot = match action {
            ShortcutAction::ToggleCompanion => &mut self.toggle_companion,
            ShortcutAction::ToggleVoiceSession => &mut self.toggle_voice_session,
            ShortcutAction::InterruptReply => &mut self.interrupt_reply,
            ShortcutAction::OpenSettings => &mut self.open_settings,
        };
        *slot = accelerator;
    }

    /// 找出与 `accelerator` 相同的**其他** action（应用内查重）。
    pub fn find_conflict(
        &self,
        action: ShortcutAction,
        accelerator: &str,
    ) -> Option<ShortcutAction> {
        ShortcutAction::ALL
            .into_iter()
            .find(|a| *a != action && self.get(*a) == Some(accelerator))
    }
}
```

`validate_accelerator` 实现：

```rust
pub fn validate_accelerator(accelerator: &str) -> Result<(), String> {
    let parts: Vec<&str> = accelerator.trim().split('+').map(str::trim).collect();
    let invalid = || "快捷键须为「修饰键 + 主键」组合，如 CmdOrCtrl+Shift+Z".to_string();
    if parts.len() < 2 || parts.iter().any(|p| p.is_empty()) {
        return Err(invalid());
    }
    let (mods, main) = parts.split_at(parts.len() - 1);
    if mods.is_empty() || !mods.iter().all(|m| MODIFIERS.contains(m)) || MODIFIERS.contains(&main[0])
    {
        return Err(invalid());
    }
    Ok(())
}
```

`src/config/mod.rs` 改为：

```rust
pub mod settings;
pub mod shortcuts;
```

`src/config/settings.rs` 的 `AppConfig`（`model_library` 字段后、245 行 `}` 前）加：

```rust
    /// 全局快捷键配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcuts: Option<ShortcutsSettings>,
```

并在 settings.rs 顶部 use 区加：`use crate::config::shortcuts::ShortcutsSettings;`

**Step 4: 跑测试确认通过**

Run: `cargo test -p zapmomo config::shortcuts 2>&1 | tail -5`
Expected: PASS（`test result: ok. 6 passed`）

**Step 5: Commit**

```bash
git add src/config/shortcuts.rs src/config/mod.rs src/config/settings.rs
git commit -m "feat(config): 新增 [shortcuts] 全局快捷键配置分节与 accelerator 校验

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: voice 会话暴露外部打断标志

**Files:**
- Modify: `src/voice/session.rs`（`VoiceSession` 加 pub 方法）

**说明（无单测的原因）:** 构造 `VoiceSession` 需要真实 KWS/ASR/TTS 模型与麦克风（`new_with_parts` 会打开设备），无法在单测里实例化；本方法是一行 getter，打断链路在 Task 9 手动验证覆盖。

**Step 1: 实现**

在 `src/voice/session.rs` 的 `impl VoiceSession` 中、`run()` 方法（207 行）之前加：

```rust
    /// 外部打断标志的克隆：宿主（Tauri 全局快捷键）持有并置位后，
    /// 会话编排循环在 Thinking/Speaking 阶段执行 `do_barge_in`（停生成/合成/播放，回 Armed）。
    pub fn barge_in_flag(&self) -> Arc<AtomicBool> {
        self.barge_in.clone()
    }
```

**Step 2: 编译验证**

Run: `cargo check -p zapmomo 2>&1 | tail -3`
Expected: 无错误

**Step 3: Commit**

```bash
git add src/voice/session.rs
git commit -m "feat(voice): 暴露 barge_in 标志供宿主外部触发打断

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: 接入 tauri-plugin-global-shortcut

**Files:**
- Modify: `src-tauri/Cargo.toml`（dependencies 加一行）
- Modify: `src-tauri/src/lib.rs`（builder 链 `.plugin(...)`）

**Step 1: 加依赖**

`src-tauri/Cargo.toml` 的 `[dependencies]` 中 `tauri-plugin-dialog = "2"` 之后加：

```toml
# 系统级全局快捷键（Rust 侧注册与分发；设置页经自定义 command 间接操作）
tauri-plugin-global-shortcut = "2"
```

**Step 2: 注册插件**

在 `src-tauri/src/lib.rs` 的 builder 链中（搜 `.plugin(tauri_plugin_dialog::init)`，在其后）加：

```rust
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
```

并在 lib.rs 顶部 use 区加：

```rust
use tauri_plugin_global_shortcut::GlobalShortcutExt;
```

**Step 3: 编译验证**

Run: `cargo check -p zapmomo-app 2>&1 | tail -3`
Expected: 无错误（首次会拉取编译新依赖，耗时几分钟属正常）

**Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.toml src-tauri/src/lib.rs src-tauri/Cargo.lock
git commit -m "feat(app): 接入 tauri-plugin-global-shortcut 插件

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: dispatch 与 set/get/clear command

**Files:**
- Modify: `src-tauri/src/lib.rs`（多处，见步骤）

**Step 1: VoiceSessionState 加 barge_in 槽**

lib.rs:113-129 的 `VoiceSessionState` 改为：

```rust
/// 语音会话线程状态：共享停止标志 + 线程句柄（仿 `ListenState`）。
struct VoiceSessionState {
    running: Arc<AtomicBool>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// 当前会话的打断标志：会话线程创建 session 后写入，线程退出时清空。
    /// 全局快捷键「打断播报」置位 → 会话循环 `do_barge_in`（停生成/合成/播放，回 Armed）。
    barge_in: Mutex<Option<Arc<AtomicBool>>>,
}
```

`new()` 与字段初始化加 `barge_in: Mutex::new(None),`。

**Step 2: start_voice_session_impl 接线**

lib.rs:1627 的 `std::thread::spawn` 闭包内，`session.run()` 调用（1641 行）前后改为：

```rust
        // 暴露打断标志给宿主（全局快捷键「打断播报」置位用）
        *app.state::<VoiceSessionState>()
            .barge_in
            .lock()
            .expect("voice barge_in lock poisoned") = Some(session.barge_in_flag());
        let result = session.run();
        running.store(false, Ordering::Relaxed);
        *app.state::<VoiceSessionState>()
            .barge_in
            .lock()
            .expect("voice barge_in lock poisoned") = None;
```

（原 `let result = session.run();` 与其后的 `running.store(false, ...)` 之间插入清空逻辑，保持其余 emit 逻辑不动。）

同时 `stop_voice_session_inner`（lib.rs:1702）在 `handle.join()` 之后补一行（防会话线程 panic 残留标志）：

```rust
    *state.barge_in.lock().expect("voice barge_in lock poisoned") = None;
```

**Step 3: stop_tts 抽 inner（供打断复用，行为不变）**

lib.rs:1078 的 `stop_tts` 重构为：

```rust
/// 停止 TTS 合成/播放的内部实现（command 与全局快捷键打断共用）。
fn stop_tts_inner(state: &TtsSynthesizeState) -> Result<(), String> {
    if !state.is_synthesizing() {
        return Err("当前没有在合成".to_string());
    }
    state.busy.store(false, Ordering::Relaxed);
    let handle = state
        .handle
        .lock()
        .expect("tts handle lock poisoned")
        .take();
    if let Some(handle) = handle {
        let _ = handle.join();
    }
    Ok(())
}

/// 停止当前 TTS 合成与播放。
#[tauri::command]
fn stop_tts(state: State<'_, TtsSynthesizeState>) -> Result<(), String> {
    stop_tts_inner(state.inner())
}
```

**Step 4: interrupt_reply + dispatch_shortcut**

在窗口管理函数区（`show_settings_window` 附近，lib.rs:2515 旁）加：

```rust
/// 打断当前回复：voice 会话运行中置位打断标志（会话线程停生成/合成/播放回 Armed）；
/// 同时兜底停独立 TTS 播放与 LLM 生成（voice 未运行但测试播放/生成中的场景）。
fn interrupt_reply(app: &AppHandle) {
    let voice = app.state::<VoiceSessionState>();
    if voice.is_running()
        && let Some(flag) = voice.barge_in.lock().expect("voice barge_in lock poisoned").clone()
    {
        flag.store(true, Ordering::Relaxed);
    }
    // 「没有在合成」不算错误：打断场景下静默跳过
    let _ = stop_tts_inner(app.state::<TtsSynthesizeState>().inner());
    if let Some(engine) = app
        .state::<LlmState>()
        .engine
        .lock()
        .expect("llm lock poisoned")
        .as_ref()
    {
        engine.cancel();
    }
}

/// 全局快捷键触发分发（复用托盘/菜单同款内部函数）。
fn dispatch_shortcut(app: &AppHandle, action: zapmomo::config::shortcuts::ShortcutAction) {
    use zapmomo::config::shortcuts::ShortcutAction;
    match action {
        ShortcutAction::ToggleCompanion => toggle_companion_window(app),
        ShortcutAction::OpenSettings => show_settings_window(app),
        ShortcutAction::InterruptReply => interrupt_reply(app),
        ShortcutAction::ToggleVoiceSession => {
            // stop 需 join 会话线程（等麦克风轮询退出）、start 有模型预检，
            // 都可能耗时：放后台线程避免阻塞快捷键回调
            let app = app.clone();
            std::thread::spawn(move || {
                let state = app.state::<VoiceSessionState>();
                let result = if state.is_running() {
                    stop_voice_session_inner(state.inner())
                } else {
                    start_voice_session_impl(app.clone(), state.inner())
                };
                if let Err(e) = result {
                    tracing::warn!("切换语音会话失败: {e}");
                }
            });
        }
    }
}
```

**Step 5: 三个 command**

同区加（沿用 `set_hide_dock_icon` 的读写惯例）：

```rust
/// 读取用户自定义快捷键（action 标识 → accelerator，仅含已绑定项）。
#[tauri::command]
fn get_shortcuts() -> Result<std::collections::HashMap<String, String>, String> {
    let shortcuts = settings::load_settings()?
        .unwrap_or_default()
        .shortcuts
        .unwrap_or_default();
    let mut map = std::collections::HashMap::new();
    for action in zapmomo::config::shortcuts::ShortcutAction::ALL {
        if let Some(acc) = shortcuts.get(action) {
            map.insert(action.as_str().to_string(), acc.to_string());
        }
    }
    Ok(map)
}

/// 绑定快捷键：校验 → 查重 → **先注册成功再落盘**（键位被系统/其他应用占用时
/// 注册失败，配置保持原值，杜绝「界面已绑定但实际不生效」的假状态）。
#[tauri::command]
fn set_shortcut(app: AppHandle, action: String, accelerator: String) -> Result<(), String> {
    use zapmomo::config::shortcuts::{validate_accelerator, ShortcutAction};
    let action = ShortcutAction::from_ident(&action)
        .ok_or_else(|| format!("未知的操作：{action}"))?;
    let accelerator = accelerator.trim().to_string();
    validate_accelerator(&accelerator)?;

    let mut cfg = settings::load_settings()?.unwrap_or_default();
    let shortcuts = cfg.shortcuts.get_or_insert_with(Default::default);
    if let Some(other) = shortcuts.find_conflict(action, &accelerator) {
        return Err(format!("该快捷键已绑定到「{}」", other.label()));
    }
    // 幂等：与当前值相同直接成功
    if shortcuts.get(action) == Some(accelerator.as_str()) {
        return Ok(());
    }
    let old = shortcuts.get(action).map(str::to_string);
    app.global_shortcut()
        .on_shortcut(accelerator.as_str(), move |app, _sc, _ev| {
            dispatch_shortcut(app, action);
        })
        .map_err(|e| format!("注册失败，可能已被其他应用占用：{e}"))?;
    // 新键注册成功后才解绑旧键
    if let Some(old) = old
        && let Err(e) = app.global_shortcut().unregister(old.as_str())
    {
        tracing::warn!("解绑旧快捷键 {old} 失败: {e}");
    }
    shortcuts.set(action, Some(accelerator));
    settings::save_settings(&cfg)?;
    Ok(())
}

/// 清除操作的快捷键绑定（解绑 + 配置置空）。
#[tauri::command]
fn clear_shortcut(app: AppHandle, action: String) -> Result<(), String> {
    use zapmomo::config::shortcuts::ShortcutAction;
    let action = ShortcutAction::from_ident(&action)
        .ok_or_else(|| format!("未知的操作：{action}"))?;
    let mut cfg = settings::load_settings()?.unwrap_or_default();
    if let Some(shortcuts) = cfg.shortcuts.as_mut() {
        if let Some(cur) = shortcuts.get(action).map(str::to_string)
            && let Err(e) = app.global_shortcut().unregister(cur.as_str())
        {
            tracing::warn!("解绑快捷键 {cur} 失败: {e}");
        }
        shortcuts.set(action, None);
    }
    settings::save_settings(&cfg)?;
    Ok(())
}
```

**Step 6: 注册 command**

`generate_handler!` 列表（lib.rs:3758 `open_settings,` 附近）加三行：

```rust
            get_shortcuts,
            set_shortcut,
            clear_shortcut,
```

**Step 7: 编译 + lint**

Run: `cargo clippy -p zapmomo-app -- -D warnings 2>&1 | tail -5`
Expected: 无警告无错误

**Step 8: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(app): 全局快捷键 set/get/clear command 与触发分发

打断播报复用 voice 会话 barge_in 标志（停生成/合成/播放回待唤醒），
并兜底停止独立 TTS 播放与 LLM 生成；绑定遵循先注册成功再落盘。

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: 启动时注册已配置的快捷键

**Files:**
- Modify: `src-tauri/src/lib.rs`（setup 闭包末尾 + 新函数）

**Step 1: 实现**

在 `dispatch_shortcut` 旁加：

```rust
/// 启动时按 `[shortcuts]` 配置注册全局快捷键：单个失败仅告警不阻塞启动
/// （键位可能已被其他软件占用），其余照常注册。
fn register_shortcuts_at_startup(app: &AppHandle) {
    use zapmomo::config::shortcuts::ShortcutAction;
    let shortcuts = settings::load_settings()
        .ok()
        .flatten()
        .and_then(|s| s.shortcuts)
        .unwrap_or_default();
    for action in ShortcutAction::ALL {
        let Some(acc) = shortcuts.get(action).map(str::to_string) else {
            continue;
        };
        let result = app.global_shortcut().on_shortcut(acc.as_str(), move |app, _sc, _ev| {
            dispatch_shortcut(app, action);
        });
        match result {
            Ok(()) => tracing::info!("全局快捷键已注册：{} = {}", action.as_str(), acc),
            Err(e) => tracing::warn!(
                "全局快捷键 {} ({}) 注册失败，已跳过: {e}",
                action.as_str(),
                acc
            ),
        }
    }
}
```

在 `setup` 闭包末尾（KWS 自动监听代码块之后、闭包 `Ok(())` 之前）加：

```rust
            // 注册用户自定义全局快捷键（[shortcuts] 分节；单个失败仅告警）
            register_shortcuts_at_startup(app.handle());
```

**Step 2: 编译验证**

Run: `cargo check -p zapmomo-app 2>&1 | tail -3`
Expected: 无错误

**Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(app): 启动时按配置注册全局快捷键

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: 前端 api 封装

**Files:**
- Modify: `src-tauri/frontend/src/lib/tauri.ts`（api 对象加 3 个方法）
- Modify: `src-tauri/frontend/src/types/tauri.ts`（加类型）

**Step 1: 类型**

`src-tauri/frontend/src/types/tauri.ts` 加：

```ts
/** 可绑定全局快捷键的操作标识（与 Rust `ShortcutAction::as_str` 一致）。 */
export type ShortcutActionId =
  | "toggle_companion"
  | "toggle_voice_session"
  | "interrupt_reply"
  | "open_settings";
```

**Step 2: api 方法**

`tauri.ts` 的 `api` 对象中 `getHideDockIcon` / `setHideDockIcon`（186-187 行）之后加（并在顶部 type import 列表加 `ShortcutActionId`）：

```ts
  getShortcuts: () => invoke<Record<string, string>>("get_shortcuts"),
  setShortcut: (args: { action: ShortcutActionId; accelerator: string }) =>
    invoke<void>("set_shortcut", args),
  clearShortcut: (args: { action: ShortcutActionId }) =>
    invoke<void>("clear_shortcut", args),
```

**Step 3: 类型检查**

Run: `cd src-tauri/frontend && pnpm exec tsc -b 2>&1 | tail -3`
Expected: 无输出（通过）。注意：必须用 `tsc -b`，不要用 `tsc --noEmit`（后者在本项目有空通过的历史坑）。

**Step 4: Commit**

```bash
git add src-tauri/frontend/src/lib/tauri.ts src-tauri/frontend/src/types/tauri.ts
git commit -m "feat(frontend): 快捷键 command 的 api 封装

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: accelerator 工具 + ShortcutsSection 组件（TDD）

**Files:**
- Create: `src-tauri/frontend/src/components/settings/accelerator.ts`
- Create: `src-tauri/frontend/src/components/settings/ShortcutsSection.tsx`
- Test: `src-tauri/frontend/src/components/settings/accelerator.test.ts`
- Test: `src-tauri/frontend/src/components/settings/ShortcutsSection.test.tsx`

**Step 1: 写 accelerator 失败测试**

`accelerator.test.ts`：

```ts
import { describe, expect, it } from "vitest";
import { acceleratorFromEvent, formatAccelerator } from "./accelerator";

const ev = (code: string, mods: Partial<KeyboardEvent> = {}) => ({
  code,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

describe("acceleratorFromEvent", () => {
  it("Cmd+Shift+V → CmdOrCtrl+Shift+V", () => {
    expect(acceleratorFromEvent(ev("KeyV", { metaKey: true, shiftKey: true }))).toBe(
      "CmdOrCtrl+Shift+V",
    );
  });

  it("Ctrl+Alt+1（Windows/Linux 风格）→ CmdOrCtrl+Alt+1", () => {
    expect(acceleratorFromEvent(ev("Digit1", { ctrlKey: true, altKey: true }))).toBe(
      "CmdOrCtrl+Alt+1",
    );
  });

  it("支持标点/空格主键（Code 名）", () => {
    expect(acceleratorFromEvent(ev("Comma", { metaKey: true }))).toBe("CmdOrCtrl+Comma");
    expect(acceleratorFromEvent(ev("Space", { ctrlKey: true }))).toBe("CmdOrCtrl+Space");
  });

  it("裸键（无修饰键）返回 null", () => {
    expect(acceleratorFromEvent(ev("KeyZ"))).toBeNull();
  });

  it("不支持的主键返回 null", () => {
    expect(acceleratorFromEvent(ev("F5", { metaKey: true }))).toBeNull();
    expect(acceleratorFromEvent(ev("Meta", { metaKey: true }))).toBeNull();
  });
});

describe("formatAccelerator", () => {
  it("mac 显示符号：CmdOrCtrl+Shift+V → ⌘⇧V", () => {
    expect(formatAccelerator("CmdOrCtrl+Shift+V", true)).toBe("⌘⇧V");
  });

  it("非 mac 显示全名：CmdOrCtrl+Shift+V → Ctrl+Shift+V", () => {
    expect(formatAccelerator("CmdOrCtrl+Shift+V", false)).toBe("Ctrl+Shift+V");
  });

  it("标点主键显示符号：CmdOrCtrl+Comma → ⌘,", () => {
    expect(formatAccelerator("CmdOrCtrl+Comma", true)).toBe("⌘,");
  });
});
```

**Step 2: 跑测试确认失败**

Run: `cd src-tauri/frontend && pnpm vitest run src/components/settings/accelerator.test.ts 2>&1 | tail -5`
Expected: FAIL（模块不存在）

**Step 3: 实现 accelerator.ts**

```ts
/**
 * 全局快捷键 accelerator 工具：KeyboardEvent → accelerator 字符串（与 Rust 侧
 * tauri-plugin-global-shortcut 格式一致：修饰键 + 主键，`+` 分隔），以及
 * accelerator → 展示文本（mac 符号 / 其他平台全名）。
 *
 * 主键格式：字母/数字用单字符（Z、1），其余用 Code 名（Comma、Space…）。
 * 修饰键统一生成 CmdOrCtrl / Alt / Shift（跨平台由插件映射到 Cmd 或 Ctrl）。
 */

export interface ShortcutLikeEvent {
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

/** code → accelerator 主键段；未列出的 code 不支持自定义（返回 null 忽略）。 */
const CODE_TO_MAIN: Record<string, string> = {
  Space: "Space",
  Comma: "Comma",
  Period: "Period",
  Slash: "Slash",
  Semicolon: "Semicolon",
  Quote: "Quote",
  BracketLeft: "BracketLeft",
  BracketRight: "BracketRight",
  Backslash: "Backslash",
  Minus: "Minus",
  Equal: "Equal",
  Backquote: "Backquote",
  Tab: "Tab",
  Enter: "Enter",
};

function mainKeyFromCode(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3); // KeyZ → Z
  if (/^Digit\d$/.test(code)) return code.slice(5); // Digit1 → 1
  return CODE_TO_MAIN[code] ?? null;
}

/** 从键盘事件构造 accelerator；裸键（无修饰键）或不支持的主键返回 null。 */
export function acceleratorFromEvent(e: ShortcutLikeEvent): string | null {
  const main = mainKeyFromCode(e.code);
  if (!main) return null;
  const mods: string[] = [];
  if (e.metaKey || e.ctrlKey) mods.push("CmdOrCtrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (mods.length === 0) return null;
  return [...mods, main].join("+");
}

const MOD_DISPLAY_WIN: Record<string, string> = {
  CmdOrCtrl: "Ctrl",
  Alt: "Alt",
  Shift: "Shift",
};

const MOD_SYMBOL_MAC: Record<string, string> = {
  CmdOrCtrl: "⌘",
  Alt: "⌥",
  Shift: "⇧",
};

const MAIN_DISPLAY: Record<string, string> = {
  Comma: ",",
  Period: ".",
  Slash: "/",
  Semicolon: ";",
  Quote: "'",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Minus: "-",
  Equal: "=",
  Backquote: "`",
  Space: "Space",
  Tab: "Tab",
  Enter: "Enter",
};

/** accelerator → 展示文本。mac 用符号拼接种类（⌘⇧V），其余平台用 + 连接全名。 */
export function formatAccelerator(accelerator: string, isMac: boolean): string {
  const parts = accelerator.split("+");
  const main = parts[parts.length - 1];
  const mods = parts.slice(0, -1);
  const mainDisplay = MAIN_DISPLAY[main] ?? main;
  if (!isMac) {
    return [...mods.map((m) => MOD_DISPLAY_WIN[m] ?? m), mainDisplay].join("+");
  }
  const symbols = mods.map((m) => MOD_SYMBOL_MAC[m] ?? m).join("");
  return `${symbols}${mainDisplay}`;
}
```

**Step 4: 跑测试确认通过**

Run: `cd src-tauri/frontend && pnpm vitest run src/components/settings/accelerator.test.ts 2>&1 | tail -3`
Expected: PASS（8 passed）

**Step 5: 写 ShortcutsSection 失败测试**

`ShortcutsSection.test.tsx`：

```tsx
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "@/components/ui/toast";
import { ShortcutsSection } from "./ShortcutsSection";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

function renderSection() {
  return render(
    <ToastProvider>
      <ShortcutsSection />
    </ToastProvider>,
  );
}

const keyDown = (code: string, mods: Partial<KeyboardEvent> = {}) =>
  fireEvent.keyDown(window, { code, key: code, ...mods });

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === "get_shortcuts") return Promise.resolve({});
    return Promise.resolve();
  });
});

describe("ShortcutsSection", () => {
  it("挂载时读取已绑定快捷键并展示", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_shortcuts")
        return Promise.resolve({ toggle_companion: "CmdOrCtrl+Shift+Z" });
      return Promise.resolve();
    });
    renderSection();
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("get_shortcuts"),
    );
    // 已绑定显示 accelerator（含主键 Z），未绑定显示「未设置」
    expect(screen.getByLabelText("设置 显示/隐藏桌宠 快捷键").textContent).toContain("Z");
    expect(screen.getByLabelText("设置 语音会话 开/关 快捷键").textContent).toContain(
      "未设置",
    );
  });

  it("录制：点击后按键组合 → 调 set_shortcut 并更新展示", async () => {
    renderSection();
    const btn = await screen.findByLabelText("设置 语音会话 开/关 快捷键");
    fireEvent.click(btn);
    expect(btn.textContent).toContain("按下组合键");
    keyDown("KeyV", { metaKey: true, shiftKey: true });
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_shortcut", {
        action: "toggle_voice_session",
        accelerator: "CmdOrCtrl+Shift+V",
      }),
    );
    await waitFor(() =>
      expect(screen.getByLabelText("设置 语音会话 开/关 快捷键").textContent).toContain("V"),
    );
  });

  it("Esc 取消录制且不发请求", async () => {
    renderSection();
    const btn = await screen.findByLabelText("设置 打开设置 快捷键");
    fireEvent.click(btn);
    keyDown("Escape");
    keyDown("KeyO", { metaKey: true });
    expect(invokeMock).not.toHaveBeenCalledWith("set_shortcut", expect.anything());
    expect(screen.getByLabelText("设置 打开设置 快捷键").textContent).toContain("未设置");
  });

  it("裸按键被忽略（等待有效组合）", async () => {
    renderSection();
    const btn = await screen.findByLabelText("设置 打开设置 快捷键");
    fireEvent.click(btn);
    keyDown("KeyO");
    expect(invokeMock).not.toHaveBeenCalledWith("set_shortcut", expect.anything());
    expect(screen.getByLabelText("设置 打开设置 快捷键").textContent).toContain("按下组合键");
  });

  it("应用内冲突：同键已绑定其他操作 → 本地拦截提示，不发请求", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_shortcuts")
        return Promise.resolve({ interrupt_reply: "CmdOrCtrl+Shift+V" });
      return Promise.resolve();
    });
    renderSection();
    await screen.findByLabelText("设置 打断播报 快捷键");
    fireEvent.click(screen.getByLabelText("设置 语音会话 开/关 快捷键"));
    keyDown("KeyV", { metaKey: true, shiftKey: true });
    await waitFor(() =>
      expect(screen.getByText(/已绑定到「打断播报」/)).toBeTruthy(),
    );
    expect(invokeMock).not.toHaveBeenCalledWith("set_shortcut", expect.anything());
  });

  it("后端注册失败：显示错误且原绑定不变", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_shortcuts")
        return Promise.resolve({ open_settings: "CmdOrCtrl+Shift+O" });
      if (cmd === "set_shortcut")
        return Promise.reject("注册失败，可能已被其他应用占用");
      return Promise.resolve();
    });
    renderSection();
    await screen.findByLabelText("设置 打开设置 快捷键");
    fireEvent.click(screen.getByLabelText("设置 打开设置 快捷键"));
    keyDown("KeyP", { metaKey: true, shiftKey: true });
    await waitFor(() =>
      expect(screen.getByText(/注册失败/)).toBeTruthy(),
    );
    expect(screen.getByLabelText("设置 打开设置 快捷键").textContent).toContain("O");
  });

  it("清除：调 clear_shortcut 并回到未设置", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_shortcuts")
        return Promise.resolve({ toggle_companion: "CmdOrCtrl+Shift+Z" });
      return Promise.resolve();
    });
    renderSection();
    fireEvent.click(await screen.findByLabelText("清除 显示/隐藏桌宠 快捷键"));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("clear_shortcut", {
        action: "toggle_companion",
      }),
    );
    await waitFor(() =>
      expect(screen.getByLabelText("设置 显示/隐藏桌宠 快捷键").textContent).toContain(
        "未设置",
      ),
    );
  });
});
```

**Step 6: 跑测试确认失败**

Run: `cd src-tauri/frontend && pnpm vitest run src/components/settings/ShortcutsSection.test.tsx 2>&1 | tail -5`
Expected: FAIL（组件不存在）

**Step 7: 实现 ShortcutsSection.tsx**

```tsx
import { Keyboard } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { useToast } from "@/components/ui/toast";
import { api } from "@/lib/tauri";
import type { ShortcutActionId } from "@/types/tauri";
import { acceleratorFromEvent, formatAccelerator } from "./accelerator";

/** 可绑定操作清单（id 与 Rust `ShortcutAction::as_str` 一致）。 */
const ACTIONS: { id: ShortcutActionId; label: string; hint: string }[] = [
  { id: "toggle_companion", label: "显示/隐藏桌宠", hint: "演示、录屏时快速藏起或召回桌宠" },
  { id: "toggle_voice_session", label: "语音会话 开/关", hint: "一键开启或关闭语音会话（麦克风）" },
  { id: "interrupt_reply", label: "打断播报", hint: "停止当前回复的生成与朗读，回到待唤醒" },
  { id: "open_settings", label: "打开设置", hint: "随时打开设置窗口" },
];

const isMac = /mac/i.test(navigator.platform || navigator.userAgent);

/**
 * 设置页「快捷键」区块：为高频操作自定义系统级全局快捷键。
 *
 * 录制态只接受「修饰键 + 字母/数字/常见标点」组合（裸键忽略，Esc 取消）；
 * 保存走后端 set_shortcut（先注册成功再落盘，失败时原绑定保持）。
 */
export function ShortcutsSection() {
  const toast = useToast();
  const [bindings, setBindings] = useState<Record<string, string>>({});
  const [recording, setRecording] = useState<ShortcutActionId | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void api
      .getShortcuts()
      .then(setBindings)
      .catch(() => {});
  }, []);

  const submit = useCallback(
    async (action: ShortcutActionId, accelerator: string) => {
      setRecording(null);
      // 本地冲突防线：同键已绑定其他操作直接拦截（后端还有兜底校验）
      const conflict = Object.entries(bindings).find(
        ([a, acc]) => a !== action && acc === accelerator,
      );
      if (conflict) {
        const label = ACTIONS.find((x) => x.id === conflict[0])?.label ?? conflict[0];
        setError(`该快捷键已绑定到「${label}」`);
        return;
      }
      try {
        await api.setShortcut({ action, accelerator });
        setBindings((prev) => ({ ...prev, [action]: accelerator }));
        setError(null);
        toast.success("快捷键已保存");
      } catch (e) {
        setError(String(e));
      }
    },
    [bindings, toast],
  );

  // 录制态：捕获 window keydown（capture 阶段，避免输入框等抢先处理）
  useEffect(() => {
    if (!recording) return;
    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setRecording(null);
        setError(null);
        return;
      }
      const accelerator = acceleratorFromEvent(e);
      if (!accelerator) return; // 裸键/不支持的主键：忽略，等待有效组合
      void submit(recording, accelerator);
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [recording, submit]);

  const clear = async (action: ShortcutActionId) => {
    try {
      await api.clearShortcut({ action });
      setBindings((prev) => {
        const next = { ...prev };
        delete next[action];
        return next;
      });
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <section className="overflow-hidden rounded-[16px] border border-panel-border bg-panel-background">
      <div className="px-3.5 py-2.5">
        <div className="flex items-center gap-2.5">
          <Keyboard className="h-4 w-4 shrink-0 text-text-secondary" />
          <div>
            <h2 className="text-base font-semibold text-text-primary">快捷键</h2>
            <p className="mt-0.5 text-xs text-text-muted">
              为高频操作设置系统级全局快捷键（任意应用中可触发）；需包含修饰键
            </p>
          </div>
        </div>
      </div>

      <dl className="divide-y divide-divider">
        {ACTIONS.map(({ id, label, hint }) => {
          const bound = bindings[id];
          const isRecording = recording === id;
          return (
            <div
              key={id}
              className="flex items-center justify-between gap-3.5 px-3.5 py-2.5"
            >
              <div className="min-w-0">
                <dt className="text-sm text-text-primary">{label}</dt>
                <dd className="mt-0.5 text-xs text-text-muted">{hint}</dd>
              </div>
              <div className="flex shrink-0 items-center gap-1.5">
                <Button
                  size="sm"
                  variant={bound ? "outline" : "ghost"}
                  aria-label={`设置 ${label} 快捷键`}
                  className={
                    bound ? undefined : "text-text-muted"
                  }
                  onClick={() => {
                    setError(null);
                    setRecording(isRecording ? null : id);
                  }}
                >
                  {isRecording
                    ? "按下组合键…（Esc 取消）"
                    : bound
                      ? formatAccelerator(bound, isMac)
                      : "未设置"}
                </Button>
                {bound && (
                  <Button
                    size="sm"
                    variant="ghost"
                    aria-label={`清除 ${label} 快捷键`}
                    onClick={() => void clear(id)}
                  >
                    清除
                  </Button>
                )}
              </div>
            </div>
          );
        })}
      </dl>

      {error && (
        <div className="px-3.5 pb-2.5 text-xs text-destructive">{error}</div>
      )}
    </section>
  );
}
```

注意：`text-destructive` 类名先看项目 Alert destructive 的实际样式类（`@/components/ui/alert`），保持一致；若项目用别的 token（如 `text-red-500`），跟随项目。

**Step 8: 跑测试确认通过**

Run: `cd src-tauri/frontend && pnpm vitest run src/components/settings 2>&1 | tail -3`
Expected: PASS（accelerator 8 + section 7，共 15 passed）

**Step 9: Commit**

```bash
git add src-tauri/frontend/src/components/settings/
git commit -m "feat(frontend): 设置页快捷键录制组件与 accelerator 工具

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 8: 挂进设置页 + 更新现有测试

**Files:**
- Modify: `src-tauri/frontend/src/pages/SettingsPage.tsx`
- Modify: `src-tauri/frontend/src/pages/SettingsPage.test.tsx`

**Step 1: 更新现有测试 mock（先红）**

`SettingsPage.test.tsx` 的 `invokeMock.mockImplementation` switch（62-73 行）加 case：

```ts
      case "get_shortcuts":
        return Promise.resolve({});
```

**Step 2: 挂载 Section**

`SettingsPage.tsx`：顶部 import `import { ShortcutsSection } from "@/components/settings/ShortcutsSection";`，在「存储位置」`</section>` 之后（页面容器内）加：

```tsx
      {/* 快捷键 */}
      <ShortcutsSection />
```

**Step 3: 跑设置页测试**

Run: `cd src-tauri/frontend && pnpm vitest run src/pages/SettingsPage.test.tsx 2>&1 | tail -3`
Expected: PASS（若挂 Section 前忘加 mock case 会失败——这正是 Step 1 先行的原因）

**Step 4: Commit**

```bash
git add src-tauri/frontend/src/pages/SettingsPage.tsx src-tauri/frontend/src/pages/SettingsPage.test.tsx
git commit -m "feat(frontend): 设置页接入快捷键区块

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 9: 全量验证与手动清单

**Step 1: Rust 全量**

```bash
cargo fmt
cargo clippy --workspace -- -D warnings 2>&1 | tail -3
cargo test -- --test-threads=1 2>&1 | tail -5
```
Expected: clippy 无警告；全部测试通过（含 Task 1 新增 6 个）。

**Step 2: 前端全量**

```bash
cd src-tauri/frontend && pnpm exec tsc -b && pnpm test:run 2>&1 | tail -3
```
Expected: 类型检查通过；全部 Vitest 套件通过。

**Step 3: 提交格式化产物（如有）**

```bash
git add -A && git commit -m "style: cargo fmt

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```
（若 fmt 无改动则跳过）

**Step 4: 手动验证清单（macOS 真机，交给用户或本机可跑 `pnpm tauri dev` 时执行）**

1. 设置页出现「快捷键」区块，四行操作均为「未设置」
2. 逐个录入（如 ⌘⇧Z / ⌘⇧V / ⌘⇧X / ⌘⇧O），对勾反馈 + `~/.zapmomo/settings.toml` 出现 `[shortcuts]` 分节
3. 聚焦其他应用（如访达）逐一按快捷键：
   - ⌘⇧Z 桌宠隐藏/再显示
   - ⌘⇧O 设置窗口弹出
   - ⌘⇧V 语音会话开关（观察右上角状态点 / 对话页 Switch）
   - ⌘⇧X 播报中打断（桌宠立即安静、回待唤醒态）
4. 重复录同键到另一操作 → 提示「已绑定到「XX」」
5. 录一个被其他应用占用的键（如与 Raycast 冲突）→ 提示注册失败，原绑定保持
6. 清除 → 键不再生效、设置页回「未设置」
7. 重启应用（设置页「重启」按钮）→ 快捷键仍生效（启动注册）
8. 不设置任何快捷键的老配置启动 → 行为无变化、日志无注册记录

**Step 5: 最终提交与收尾**

确认工作树干净后，按 `superpowers:finishing-a-development-branch` 流程走（PR 标题建议 `feat(app): 设置页支持自定义全局快捷键 (#issue)`）。
