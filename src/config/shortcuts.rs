//! 全局快捷键配置：`[shortcuts]` 分节与 action 定义。
//!
//! accelerator 为 tauri-plugin-global-shortcut 标准格式（修饰键 + 主键，`+` 分隔，
//! 如 `CmdOrCtrl+Shift+Z`）。所有字段缺省 `None` = 不注册任何全局快捷键（默认策略，
//! 老用户升级零变化）；注册/解绑在 tauri 侧完成，本模块只管配置与纯逻辑校验。

use serde::{Deserialize, Serialize};

/// 可绑定全局快捷键的操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShortcutAction {
    /// 打开设置窗口
    OpenSettings,
}

/// 快捷键配置分节（action → accelerator；`None` = 未绑定）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ShortcutsSettings {
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
    let parts: Vec<&str> = accelerator.trim().split('+').map(str::trim).collect();
    let invalid = || "快捷键须为「修饰键 + 主键」组合，如 CmdOrCtrl+Shift+Z".to_string();
    if parts.len() < 2 || parts.iter().any(|p| p.is_empty()) {
        return Err(invalid());
    }
    let (mods, main) = parts.split_at(parts.len() - 1);
    if mods.is_empty()
        || !mods.iter().all(|m| MODIFIERS.contains(m))
        || MODIFIERS.contains(&main[0])
    {
        return Err(invalid());
    }
    Ok(())
}

impl ShortcutAction {
    /// 全部可绑定操作（配置遍历 / 启动注册用）。
    pub const ALL: [ShortcutAction; 1] = [ShortcutAction::OpenSettings];

    /// snake_case 标识：配置字段名 / 前端 command 参数。
    pub fn as_str(self) -> &'static str {
        match self {
            ShortcutAction::OpenSettings => "open_settings",
        }
    }

    /// 中文标签（错误文案「已绑定到 XX」用，与设置页展示一致）。
    pub fn label(self) -> &'static str {
        match self {
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
            ShortcutAction::OpenSettings => self.open_settings.as_deref(),
        }
    }

    pub fn set(&mut self, action: ShortcutAction, accelerator: Option<String>) {
        let slot = match action {
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
        assert_eq!(s.get(ShortcutAction::OpenSettings), None);
        s.set(
            ShortcutAction::OpenSettings,
            Some("CmdOrCtrl+Shift+Z".into()),
        );
        assert_eq!(
            s.get(ShortcutAction::OpenSettings),
            Some("CmdOrCtrl+Shift+Z")
        );
        s.set(ShortcutAction::OpenSettings, None);
        assert_eq!(s.get(ShortcutAction::OpenSettings), None);
    }

    #[test]
    fn test_find_conflict() {
        let mut s = ShortcutsSettings::default();
        s.set(
            ShortcutAction::OpenSettings,
            Some("CmdOrCtrl+Shift+O".into()),
        );
        // 同 action 自身 → 不算冲突
        assert_eq!(
            s.find_conflict(ShortcutAction::OpenSettings, "CmdOrCtrl+Shift+O"),
            None
        );
    }

    #[test]
    fn test_toml_roundtrip_and_default_absent() {
        // 含 [shortcuts] 的配置可解析；未写分节时为 None（老配置兼容）
        let with_section = r#"
[shortcuts]
open_settings = "CmdOrCtrl+Shift+O"
"#;
        let cfg: crate::config::settings::AppConfig = toml::from_str(with_section).unwrap();
        assert_eq!(
            cfg.shortcuts.unwrap().open_settings.as_deref(),
            Some("CmdOrCtrl+Shift+O")
        );

        let empty: crate::config::settings::AppConfig = toml::from_str("").unwrap();
        assert!(empty.shortcuts.is_none());

        // 序列化：未绑定字段不落盘
        let mut s = ShortcutsSettings::default();
        s.set(
            ShortcutAction::OpenSettings,
            Some("CmdOrCtrl+Shift+O".into()),
        );
        let out = toml::to_string(&s).unwrap();
        assert!(out.contains("open_settings"));
    }
}
