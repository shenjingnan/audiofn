# dsh-bridge（ZapMomo × deepseek-harness 桌宠感知桥）实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** ZapMomo 桌宠通过 loopback HTTP 直推实时感知 deepseek-harness（dsh）任务状态，任务开始/完成/失败时以气泡 + 语音播报固定模板台词。

**Architecture:** dsh 侧薄 Cordis 插件 POST 语义化事件到 ZapMomo 内嵌的 tiny_http 桥（127.0.0.1 随机端口 + 发现文件 + token）；根 crate `src/dsh/` 承载全部纯逻辑（解析/节流/台词/server），src-tauri 只做接线（State/setup/commands/TTS/落盘）；前端新建桌宠气泡组件消费 `dsh-speak` 事件。

**Tech Stack:** Rust（tiny_http 0.12、既有 sherpa TTS + rodio）、React 19 + Tailwind + shadcn/ui、Vitest 4、Cordis 插件（TypeScript）。

**设计文档:** `docs/plans/2026-08-21-dsh-bridge-design.md`（五节均已与用户确认）

---

## 全局约定

- 所有命令在 worktree 根目录执行：`/Users/nemo/Projects/shenjingnan/zapmomo/.claude/worktrees/sleepy-bubbling-narwhal`
- **Rust 测试**：`cargo test -- --test-threads=1`（项目约定，避免 HOME env 竞争）
- **worktree 编译兜底**：若 `sherpa-onnx-sys` 下载失败（SOCKS 代理问题），设
  `SHERPA_ONNX_LIB_DIR=/Users/nemo/Projects/shenjingnan/zapmomo/target/debug/build/sherpa-onnx-sys-0ade50708362e19c/out`
- **前端测试**：`pnpm -C src-tauri/frontend test:run`；**前端 Lint**：`pnpm -C src-tauri/frontend check`（biome）
- **三件套**：`cargo fmt --check && cargo clippy -- -D warnings && cargo test -- --test-threads=1`
- **提交规范**：Conventional Commits，中文描述，结尾加
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`

---

## 阶段 1：桥 + 气板（无语音）

### Task 1: 根 crate 新增 `[dsh]` 配置段

**Files:**
- Modify: `Cargo.toml`（根，[dependencies] 增加 tiny_http）
- Modify: `src/lib.rs:14`（模块声明区增加 `pub mod dsh;`，按字母序放 `config` 之后）
- Create: `src/dsh/mod.rs`（本任务先建骨架）
- Create: `src/dsh/config.rs`
- Modify: `src/config/settings.rs`（AppConfig 增加字段 + DshSettings 结构 + Default + 测试）

**Step 1: 加依赖与模块骨架**

`Cargo.toml` [dependencies] 段（`sysinfo` 之后）：

```toml
# dsh 桥：loopback HTTP 接收 deepseek-harness 插件推送的任务事件
tiny_http = "0.12"
```

`src/dsh/mod.rs`（本任务骨架，后续任务逐步充实）：

```rust
/// dsh 桥：接收 deepseek-harness 插件推送的任务事件（loopback HTTP 直推）。
///
/// ZapMomo 在 app 进程内起一个仅绑 127.0.0.1 的极小 HTTP 服务；dsh 侧 Cordis 插件
/// 在任务状态翻转瞬间 POST 语义化事件（`POST /dsh/events` + Bearer token），毫秒级
/// 到达、无轮询。端口与 token 写入发现文件 `~/.zapmomo/runtime/dsh-bridge.json`
/// （权限 0600），插件每次发送前现读；ZapMomo 未运行时插件静默跳过。
pub mod config;
```

`src/lib.rs` 模块声明（按现有字母序，`datetime` 与 `kws` 之间）：

```rust
pub mod dsh;
```

**Step 2: 写失败测试**（settings.rs 底部 `mod tests` 内追加）

```rust
#[test]
fn test_parse_dsh_section() {
    run_with_temp_home(|_| {
        std::fs::create_dir_all(get_settings_dir()).unwrap();
        std::fs::write(
            get_settings_path(),
            "[dsh]\nenabled = false\nport = 47800\nvoice_enabled = false\n",
        )
        .unwrap();
        let cfg = load_settings().unwrap().unwrap();
        let dsh = cfg.dsh.expect("[dsh] 段应解析");
        assert_eq!(dsh.enabled, Some(false));
        assert_eq!(dsh.port, Some(47800));
        assert_eq!(dsh.voice_enabled, Some(false));
        assert_eq!(dsh.record_to_history, None);
    });
}

#[test]
fn test_dsh_section_absent_defaults_none() {
    run_with_temp_home(|home| {
        write_toml_settings(home, "debug = true\n");
        let cfg = load_settings().unwrap().unwrap();
        assert!(cfg.dsh.is_none());
    });
}
```

**Step 3: 跑测试确认失败**

Run: `cargo test test_parse_dsh_section -- --test-threads=1`
Expected: 编译失败（`cfg.dsh` 字段不存在）

**Step 4: 最小实现**

`src/config/settings.rs`——`AppConfig` 结构体 `shortcuts` 字段后追加：

```rust
    /// dsh 桥配置（接收 deepseek-harness 插件推送的任务事件）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsh: Option<DshSettings>,
```

`Default for AppConfig` 的 `shortcuts: None,` 后追加 `dsh: None,`。

`VoiceSettings` 定义之后新增结构体：

```rust
/// dsh 桥配置。
///
/// 全部字段可缺省：未配置的项在解析时回退到 `dsh::config` 的内置默认值。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DshSettings {
    /// 是否启用桥服务（loopback HTTP 监听），缺省 true
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// 监听端口，0 = 随机端口（默认，避免冲突），缺省 0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// 事件是否语音播报（voice 会话运行中只出气泡），缺省 true
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_enabled: Option<bool>,
    /// 事件是否写入对话记录，缺省 true
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_to_history: Option<bool>,
}
```

`src/dsh/config.rs`：

```rust
/// [dsh] 配置解析：未配置项回退内置默认值。
use crate::config::settings::DshSettings;

/// 解析后的 dsh 桥配置（全字段非 Option）。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDshConfig {
    pub enabled: bool,
    /// 监听端口，0 = 随机
    pub port: u16,
    pub voice_enabled: bool,
    pub record_to_history: bool,
}

pub fn resolve(settings: Option<&DshSettings>) -> ResolvedDshConfig {
    ResolvedDshConfig {
        enabled: settings.and_then(|s| s.enabled).unwrap_or(true),
        port: settings.and_then(|s| s.port).unwrap_or(0),
        voice_enabled: settings.and_then(|s| s.voice_enabled).unwrap_or(true),
        record_to_history: settings.and_then(|s| s.record_to_history).unwrap_or(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_when_none() {
        let c = resolve(None);
        assert!(c.enabled);
        assert_eq!(c.port, 0);
        assert!(c.voice_enabled);
        assert!(c.record_to_history);
    }

    #[test]
    fn test_overrides() {
        let s = DshSettings {
            enabled: Some(false),
            port: Some(47800),
            voice_enabled: Some(false),
            record_to_history: Some(false),
        };
        let c = resolve(Some(&s));
        assert!(!c.enabled);
        assert_eq!(c.port, 47800);
        assert!(!c.voice_enabled);
        assert!(!c.record_to_history);
    }
}
```

**Step 5: 跑测试确认通过**

Run: `cargo test dsh -- --test-threads=1 && cargo test test_parse_dsh_section -- --test-threads=1`
Expected: 全部 PASS

**Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/dsh/ src/config/settings.rs
git commit -m "feat(dsh): 新增 [dsh] 配置段与解析（enabled/port/voice_enabled/record_to_history）"
```

---

### Task 2: `src/dsh/event.rs`——事件类型与宽容解析

**Files:**
- Create: `src/dsh/event.rs`
- Modify: `src/dsh/mod.rs`（声明 `pub mod event;`）

**Step 1: 写失败测试**（event.rs 底部）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_started_full() {
        let ev = parse_event(
            r#"{"type":"task-started","session_id":"s1","title":"修复登录超时","extra":"未知字段"}"#,
        )
        .unwrap()
        .expect("已知类型应返回 Some");
        assert_eq!(
            ev,
            DshEvent::TaskStarted {
                session_id: "s1".to_string(),
                title: Some("修复登录超时".to_string()),
            }
        );
        assert_eq!(ev.kind(), "task-started");
        assert_eq!(ev.session_id(), "s1");
        assert_eq!(ev.title(), Some("修复登录超时"));
    }

    #[test]
    fn test_parse_failed_truncates_detail() {
        let long = "x".repeat(300);
        let ev = parse_event(&format!(
            r#"{{"type":"task-failed","session_id":"s2","detail":"{long}"}}"#
        ))
        .unwrap()
        .unwrap();
        match ev {
            DshEvent::TaskFailed { detail, .. } => {
                assert_eq!(detail.as_deref().map(str::len), Some(200));
            }
            other => panic!("应为 TaskFailed: {other:?}"),
        }
    }

    #[test]
    fn test_parse_unknown_type_returns_none() {
        assert!(
            parse_event(r#"{"type":"todo-changed","session_id":"s"}"#)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_empty_title_treated_as_missing() {
        let ev = parse_event(r#"{"type":"task-started","session_id":"s","title":"  "}"#)
            .unwrap()
            .unwrap();
        assert_eq!(ev.title(), None);
    }

    #[test]
    fn test_parse_invalid_json_errs() {
        assert!(parse_event("不是json").is_err());
        assert!(parse_event(r#""裸字符串""#).is_err());
        // 数组载荷：Value 化后 serde 派生 visit_seq 被打开，对象守卫必须拦截
        assert!(parse_event("[1]").is_err());
        assert!(parse_event(r#"["task-started","s1"]"#).is_err());
    }

    #[test]
    fn test_all_kinds() {
        for (body, kind) in [
            (
                r#"{"type":"task-started","session_id":"s"}"#,
                "task-started",
            ),
            (
                r#"{"type":"task-finished","session_id":"s"}"#,
                "task-finished",
            ),
            (r#"{"type":"task-failed","session_id":"s"}"#, "task-failed"),
            (
                r#"{"type":"task-interrupted","session_id":"s"}"#,
                "task-interrupted",
            ),
        ] {
            assert_eq!(parse_event(body).unwrap().unwrap().kind(), kind);
        }
    }

    #[test]
    fn test_serialize_type_tag_matches_kind() {
        // 判别串存在于三处（serde rename_all 推导 / kind() / parse_event match），
        // 此测试锁定序列化 tag 与 kind() 的一致性契约
        for ev in [
            DshEvent::TaskStarted {
                session_id: "s".to_string(),
                title: None,
            },
            DshEvent::TaskFinished {
                session_id: "s".to_string(),
                title: None,
                reason: None,
            },
            DshEvent::TaskFailed {
                session_id: "s".to_string(),
                title: None,
                reason: None,
                detail: None,
            },
            DshEvent::TaskInterrupted {
                session_id: "s".to_string(),
                title: None,
                reason: None,
            },
        ] {
            assert_eq!(
                serde_json::to_value(&ev).unwrap()["type"].as_str(),
                Some(ev.kind())
            );
        }
    }

    #[test]
    fn test_field_type_drift_only_drops_field() {
        // dsh 原生 turn/end 的 reason 是对象 {"kind":...}，类型漂移只丢字段不丢事件
        let ev = parse_event(
            r#"{"type":"task-finished","session_id":"s","title":5,"reason":{"kind":"completed"}}"#,
        )
        .unwrap()
        .unwrap();
        match ev {
            DshEvent::TaskFinished {
                session_id,
                title,
                reason,
            } => {
                assert_eq!(session_id, "s");
                assert_eq!(title, None);
                assert_eq!(reason, None);
            }
            other => panic!("应为 TaskFinished: {other:?}"),
        }
        // session_id 类型漂移 → 退化为空串，事件仍解析成功
        let ev = parse_event(r#"{"type":"task-started","session_id":123}"#)
            .unwrap()
            .unwrap();
        assert_eq!(ev.session_id(), "");
    }

    #[test]
    fn test_null_type_treated_as_unknown() {
        assert!(
            parse_event(r#"{"type":null,"session_id":"s"}"#)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_all_fields_drift_still_delivers() {
        // 全字段漂移组合：session_id 数字/title 数字/reason 空对象 → 全部降级，事件仍到达
        let ev = parse_event(
            r#"{"type":"task-finished","session_id":123,"title":5,"reason":{}}"#,
        )
        .unwrap()
        .unwrap();
        match ev {
            DshEvent::TaskFinished {
                session_id,
                title,
                reason,
            } => {
                assert_eq!(session_id, "");
                assert_eq!(title, None);
                assert_eq!(reason, None);
            }
            other => panic!("应为 TaskFinished: {other:?}"),
        }
    }

    #[test]
    fn test_empty_object_returns_none() {
        // 空对象全默认：type 为 null → 按未知类型忽略
        assert!(parse_event("{}").unwrap().is_none());
    }

    #[test]
    fn test_missing_session_id_defaults_empty() {
        // 已知行为：空 session_id 会让节流 key 退化为 ("", kind)，3s 窗口内不同任务互相吞
        let ev = parse_event(r#"{"type":"task-started"}"#).unwrap().unwrap();
        assert_eq!(ev.session_id(), "");
    }

    #[test]
    fn test_detail_truncation_cjk_safe() {
        // 按字符（非字节）截断：CJK 多字节字符不被切断、不产生 replacement char
        let long = "汉".repeat(300);
        let ev = parse_event(&format!(
            r#"{{"type":"task-failed","session_id":"s","detail":"{long}"}}"#
        ))
        .unwrap()
        .unwrap();
        match ev {
            DshEvent::TaskFailed { detail, .. } => {
                let d = detail.expect("CJK detail 截断后仍应保留");
                assert_eq!(d.chars().count(), 200);
                assert!(d.chars().all(|c| c == '汉'));
            }
            other => panic!("应为 TaskFailed: {other:?}"),
        }
    }
}
```

**Step 2: 跑测试确认失败**

Run: `cargo test dsh::event -- --test-threads=1`
Expected: 编译失败（模块不存在）

**Step 3: 实现**

```rust
/// dsh 桥事件：deepseek-harness 插件推送到 `/dsh/events` 的语义化任务事件。
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 任务事件（序列化为 kebab-case `type` 判别字段，与前端 `DshEventInfo` 对应）。
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum DshEvent {
    /// 会话 idle → running（dsh `agent/status`）
    TaskStarted {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    /// turn 结束且 reason.kind = completed
    TaskFinished {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// turn 结束且 reason.kind = error
    TaskFailed {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// turn 结束且 reason.kind 为 aborted/interrupted/max-tokens/blocked 等
    TaskInterrupted {
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

impl DshEvent {
    /// 事件类型名（kebab-case，节流 key / 日志用）。
    pub fn kind(&self) -> &'static str {
        match self {
            Self::TaskStarted { .. } => "task-started",
            Self::TaskFinished { .. } => "task-finished",
            Self::TaskFailed { .. } => "task-failed",
            Self::TaskInterrupted { .. } => "task-interrupted",
        }
    }

    /// 会话 id（节流 key 用）。
    pub fn session_id(&self) -> &str {
        match self {
            Self::TaskStarted { session_id, .. }
            | Self::TaskFinished { session_id, .. }
            | Self::TaskFailed { session_id, .. }
            | Self::TaskInterrupted { session_id, .. } => session_id,
        }
    }

    /// 任务标题（模板台词用）。
    pub fn title(&self) -> Option<&str> {
        match self {
            Self::TaskStarted { title, .. }
            | Self::TaskFinished { title, .. }
            | Self::TaskFailed { title, .. }
            | Self::TaskInterrupted { title, .. } => title.as_deref(),
        }
    }
}

/// 宽容解析用的原始载荷：全字段可缺省且类型漂移不致命（`Value` 逐字段提取），
/// 未知 `type` 不报错（前向兼容）。
#[derive(Debug, Deserialize)]
struct RawEvent {
    #[serde(default)]
    r#type: Value,
    #[serde(default)]
    session_id: Value,
    #[serde(default)]
    title: Option<Value>,
    #[serde(default)]
    reason: Option<Value>,
    #[serde(default)]
    detail: Option<Value>,
}

/// 从 JSON 值中提取字符串（null / 类型漂移 → None，字段坏只丢字段不丢事件）。
fn value_as_string(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

/// 规范化文本字段：trim 首尾空白（不进气泡文本）、截断 200 字符（多生产者护栏），
/// 空白视为缺失。
fn normalize_text(s: Option<String>) -> Option<String> {
    let s = s?.trim().to_owned();
    (!s.is_empty()).then(|| s.chars().take(200).collect())
}

/// 解析一条事件载荷（逐字段宽容：字段类型漂移只丢字段不丢事件）。
///
/// - 非法 JSON / 非 JSON 对象 → `Err`（HTTP 层回 400）
/// - `type` 缺失/null/非字符串或未知值 → `Ok(None)`（调用方记 debug 后忽略）
/// - 已知 `type` → 规范化为 [`DshEvent`]：文本字段 trim + 截断 200 字符、空白视为
///   缺失；`session_id` 类型漂移退化为空串（节流 key 退化为 `("", kind)`，属已知行为）
pub fn parse_event(body: &str) -> Result<Option<DshEvent>, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|e| format!("事件载荷不是合法 JSON 对象: {e}"))?;
    if !value.is_object() {
        return Err(format!("事件载荷不是 JSON 对象: {value}"));
    }
    let RawEvent {
        r#type,
        session_id,
        title,
        reason,
        detail,
    } = serde_json::from_value(value).map_err(|e| format!("事件载荷不是合法 JSON 对象: {e}"))?;
    // `type` 非字符串（null/数字/对象）按未知类型处理，整条忽略而非报错
    let Some(r#type) = value_as_string(&r#type) else {
        return Ok(None);
    };
    let session_id = value_as_string(&session_id).unwrap_or_default();
    let title = normalize_text(title.as_ref().and_then(value_as_string));
    let reason = normalize_text(reason.as_ref().and_then(value_as_string));
    let detail = normalize_text(detail.as_ref().and_then(value_as_string));
    Ok(match r#type.as_str() {
        "task-started" => Some(DshEvent::TaskStarted { session_id, title }),
        "task-finished" => Some(DshEvent::TaskFinished {
            session_id,
            title,
            reason,
        }),
        "task-failed" => Some(DshEvent::TaskFailed {
            session_id,
            title,
            reason,
            detail,
        }),
        "task-interrupted" => Some(DshEvent::TaskInterrupted {
            session_id,
            title,
            reason,
        }),
        _ => None,
    })
}
```

`src/dsh/mod.rs` 声明区追加 `pub mod event;`。

**Step 4: 跑测试确认通过**

Run: `cargo test dsh::event -- --test-threads=1`
Expected: PASS

**Step 5: Commit**

```bash
git add src/dsh/
git commit -m "feat(dsh): 事件类型 DshEvent 与宽容解析（未知 type 忽略、detail 截断）"
```

---

### Task 3: `src/dsh/lines.rs`——模板台词

**Files:**
- Create: `src/dsh/lines.rs`
- Modify: `src/dsh/mod.rs`（声明 `pub mod lines;`）

**Step 1: 写失败测试**（lines.rs 底部）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsh::event::DshEvent;

    fn ev(kind: &str, title: Option<&str>) -> DshEvent {
        let session_id = "s".to_string();
        let title = title.map(str::to_string);
        match kind {
            "task-started" => DshEvent::TaskStarted { session_id, title },
            "task-finished" => DshEvent::TaskFinished {
                session_id,
                title,
                reason: None,
            },
            "task-failed" => DshEvent::TaskFailed {
                session_id,
                title,
                reason: None,
                detail: None,
            },
            _ => DshEvent::TaskInterrupted {
                session_id,
                title,
                reason: None,
            },
        }
    }

    #[test]
    fn test_pick_line_with_title_contains_title() {
        let line = pick_line(&ev("task-finished", Some("修复登录超时")), 0.0);
        assert!(line.contains("修复登录超时"), "台词应含标题: {line}");
        assert!(!line.contains("{t}"), "占位符应被替换: {line}");
    }

    #[test]
    fn test_pick_line_without_title_uses_plain() {
        let line = pick_line(&ev("task-finished", None), 0.0);
        assert!(!line.contains("null") && !line.is_empty());
    }

    #[test]
    fn test_roll_selects_variants_and_clamps() {
        let e = ev("task-started", Some("T"));
        let first = pick_line(&e, 0.0);
        let last = pick_line(&e, 1.0);
        assert_eq!(pick_line(&e, -0.5), first, "roll 越界 clamp 到首句");
        assert_eq!(pick_line(&e, 5.0), last, "roll 越界 clamp 到末句");
    }

    #[test]
    fn test_next_roll_in_range() {
        for _ in 0..100 {
            let r = next_roll();
            assert!((0.0..1.0).contains(&r), "roll 越界: {r}");
        }
    }
}
```

**Step 2: 跑测试确认失败**

Run: `cargo test dsh::lines -- --test-threads=1`
Expected: 编译失败

**Step 3: 实现**

```rust
/// dsh 事件的模板台词（固定模板起步；LLM 生成留待后续抽象）。
use super::event::DshEvent;
use std::sync::atomic::{AtomicU64, Ordering};

/// 有标题变体（`{t}` = 任务标题占位符）
const STARTED: &[&str] = &[
    "「{t}」开工啦，我会盯着你的～",
    "新任务「{t}」来了，冲鸭！",
    "收到，「{t}」跑起来了，去忙别的吧。",
];
/// 无标题变体
const STARTED_PLAIN: &[&str] = &["任务开工啦，我会盯着你的～", "新任务跑起来了，冲鸭！", "收到收到，盯上了～"];

const FINISHED: &[&str] = &[
    "「{t}」搞定啦！",
    "「{t}」跑完了，结果不错哦～",
    "叮～「{t}」完成了，夸夸你！",
];
const FINISHED_PLAIN: &[&str] = &["任务搞定啦！", "跑完了跑完了，一切正常～", "叮～任务完成！"];

const FAILED: &[&str] = &[
    "唔……「{t}」失败了，要不要看看日志？",
    "「{t}」出错了，抱抱你，别灰心。",
    "哎呀，「{t}」没跑成，检查一下？",
];
const FAILED_PLAIN: &[&str] =
    &["唔……任务失败了，要不要看看日志？", "任务出错了，抱抱你。", "哎呀，没跑成，检查一下？"];

const INTERRUPTED: &[&str] = &["「{t}」先停下来了～", "「{t}」被中断了，等你回来。"];
const INTERRUPTED_PLAIN: &[&str] = &["任务先停下来了～", "被中断了，等你回来。"];

/// roll 计数器：黄金比例散列（无 rand 依赖；测试用显式 roll 注入）。
static ROLL_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 下一次 pick_line 用的 roll 值（0.0..1.0，单调循环不重复）。
pub fn next_roll() -> f32 {
    let n = ROLL_COUNTER.fetch_add(1, Ordering::Relaxed);
    ((n.wrapping_mul(2654435761) % 997) as f32) / 997.0
}

/// 按事件类型选一句台词。
///
/// `roll`（0.0..1.0）决定同列表内选哪句（越界 clamp）；有 `title` 用带标题变体。
pub fn pick_line(event: &DshEvent, roll: f32) -> String {
    let (titled, plain) = match event {
        DshEvent::TaskStarted { .. } => (STARTED, STARTED_PLAIN),
        DshEvent::TaskFinished { .. } => (FINISHED, FINISHED_PLAIN),
        DshEvent::TaskFailed { .. } => (FAILED, FAILED_PLAIN),
        DshEvent::TaskInterrupted { .. } => (INTERRUPTED, INTERRUPTED_PLAIN),
    };
    let candidates: Vec<String> = match event.title() {
        Some(t) => titled.iter().map(|s| s.replace("{t}", t)).collect(),
        None => plain.iter().map(|s| s.to_string()).collect(),
    };
    let idx = ((roll.clamp(0.0, 0.9999) * candidates.len() as f32) as usize)
        .min(candidates.len() - 1);
    candidates[idx].clone()
}
```

`src/dsh/mod.rs` 声明区追加 `pub mod lines;`。

**Step 4: 跑测试确认通过** → `cargo test dsh::lines -- --test-threads=1`

**Step 5: Commit**

```bash
git add src/dsh/
git commit -m "feat(dsh): 固定模板台词表与 pick_line（有/无标题变体、roll 可注入）"
```

---

### Task 4: `src/dsh/mod.rs`——发现文件、token、节流

**Files:**
- Modify: `src/dsh/mod.rs`

**Step 1: 写失败测试**（mod.rs 底部）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsh::event::DshEvent;
    use crate::test_util::run_with_temp_home;

    #[test]
    fn test_discovery_roundtrip_and_permissions() {
        run_with_temp_home(|_| {
            let info = DiscoveryInfo {
                port: 47800,
                token: "abc".to_string(),
            };
            write_discovery(&info).unwrap();
            let read: DiscoveryInfo =
                serde_json::from_str(&std::fs::read_to_string(discovery_file()).unwrap()).unwrap();
            assert_eq!(read.port, 47800);
            assert_eq!(read.token, "abc");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(discovery_file())
                    .unwrap()
                    .permissions()
                    .mode();
                assert_eq!(mode & 0o777, 0o600, "发现文件权限应为 0600");
            }
            remove_discovery();
            assert!(!discovery_file().exists());
        });
    }

    #[test]
    fn test_generate_token_shape() {
        let a = generate_token();
        let b = generate_token();
        assert_eq!(a.len(), 32, "token 应为 32 位 hex");
        assert_ne!(a, b, "连续生成的 token 应不同");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_throttle_blocks_same_key_within_window() {
        let t = EventThrottle::new(std::time::Duration::from_secs(3));
        let ev = DshEvent::TaskStarted {
            session_id: "s1".to_string(),
            title: None,
        };
        assert!(t.allow(&ev), "首次应放行");
        assert!(!t.allow(&ev), "窗口内同 (session, kind) 应拦截");
    }

    #[test]
    fn test_throttle_different_keys_pass() {
        let t = EventThrottle::new(std::time::Duration::from_secs(3));
        let a = DshEvent::TaskStarted { session_id: "s1".to_string(), title: None };
        let b = DshEvent::TaskStarted { session_id: "s2".to_string(), title: None };
        let c = DshEvent::TaskFinished { session_id: "s1".to_string(), title: None, reason: None };
        assert!(t.allow(&a));
        assert!(t.allow(&b), "不同 session 不拦截");
        assert!(t.allow(&c), "同 session 不同类型不拦截");
    }

    #[test]
    fn test_throttle_allows_after_window() {
        let t = EventThrottle::new(std::time::Duration::from_millis(20));
        let ev = DshEvent::TaskStarted { session_id: "s1".to_string(), title: None };
        assert!(t.allow(&ev));
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(t.allow(&ev), "窗口过后应放行");
    }
}
```

**Step 2: 跑测试确认失败** → `cargo test dsh::tests -- --test-threads=1`（编译失败）

**Step 3: 实现**（mod.rs 声明区之后追加）

```rust
use crate::config::settings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 事件载荷大小上限（超出回 413）。
pub const MAX_BODY_BYTES: u64 = 64 * 1024;
/// HTTP recv 超时 = 停止标志检查周期。
pub const RECV_TIMEOUT: Duration = Duration::from_millis(200);

/// 发现文件路径：`~/.zapmomo/runtime/dsh-bridge.json`。
pub fn discovery_file() -> std::path::PathBuf {
    settings::get_settings_dir().join("runtime").join("dsh-bridge.json")
}

/// 发现文件内容（dsh 插件读取以定位桥端口与鉴权 token）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryInfo {
    pub port: u16,
    pub token: String,
}

/// 写发现文件（临时文件 + rename 原子替换；unix 下权限 0600，Windows 跳过 chmod）。
pub fn write_discovery(info: &DiscoveryInfo) -> Result<(), String> {
    let path = discovery_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建 runtime 目录失败: {e}"))?;
    }
    let body = serde_json::to_string(info).map_err(|e| format!("序列化发现文件失败: {e}"))?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &body).map_err(|e| format!("写入发现文件失败: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, &path).map_err(|e| format!("替换发现文件失败: {e}"))?;
    Ok(())
}

/// 删除发现文件（退出清理 / 启动清陈旧残留；失败仅记 debug 日志）。
pub fn remove_discovery() {
    if let Err(e) = std::fs::remove_file(discovery_file()) {
        tracing::debug!("删除发现文件失败: {e}");
    }
}

/// 生成一次性 token：sha256(纳秒时钟 ‖ pid ‖ 计数器) 十六进制前 32 位。
pub fn generate_token() -> String {
    use sha2::{Digest, Sha256};
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = Sha256::new();
    hasher.update(nanos.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    hasher.update(n.to_le_bytes());
    let full = hex::encode(hasher.finalize());
    full.chars().take(32).collect()
}

/// (session_id, 事件类型) 级别节流：窗口内重复事件直接丢弃。
///
/// 事件风暴 / dsh 重启重放的护栏；顺带清理过期项防 map 无界增长。
pub struct EventThrottle {
    window: Duration,
    last: Mutex<HashMap<(String, &'static str), Instant>>,
}

impl EventThrottle {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            last: Mutex::new(HashMap::new()),
        }
    }

    /// 该事件是否放行（窗口内同 (session, kind) 重复 → false）。
    pub fn allow(&self, event: &event::DshEvent) -> bool {
        let key = (event.session_id().to_string(), event.kind());
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        last.retain(|_, t| now.duration_since(*t) < self.window);
        match last.get(&key) {
            Some(t) if now.duration_since(*t) < self.window => false,
            _ => {
                last.insert(key, now);
                true
            }
        }
    }
}
```

**Step 4: 跑测试确认通过** → `cargo test dsh::tests -- --test-threads=1`

**Step 5: Commit**

```bash
git add src/dsh/
git commit -m "feat(dsh): 发现文件（0600）、一次性 token 与事件节流器"
```

---

### Task 5: `serve`——tiny_http 桥服务与集成测试

**Files:**
- Modify: `src/dsh/mod.rs`

**Step 1: 写失败测试**（mod.rs tests 内追加；ureq 已是根 crate 依赖）

测试用 helpers（三个测试共用）：

```rust
    /// 起 serve 线程（随机端口 + "test-token"），返回 (线程句柄, 停止标志, 事件接收器, 实际端口)。
    fn spawn_serve() -> (
        std::thread::JoinHandle<()>,
        std::sync::Arc<std::sync::atomic::AtomicBool>,
        std::sync::mpsc::Receiver<DshEvent>,
        u16,
    ) {
        let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let (event_tx, event_rx) = std::sync::mpsc::channel::<DshEvent>();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<u16>();
        let r = running.clone();
        let handle = std::thread::spawn(move || {
            let mut sink = move |ev: DshEvent| {
                let _ = event_tx.send(ev);
            };
            let mut on_ready = |port: u16| {
                let _ = ready_tx.send(port);
            };
            serve(0, "test-token", &mut sink, &r, &mut on_ready).unwrap();
        });
        let port = ready_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("on_ready 应回报端口");
        (handle, running, event_rx, port)
    }

    /// 轮询断言 serve 线程在 timeout 内退出后再 join（先 is_finished 再 join，防测试自身挂死）。
    fn join_with_deadline(handle: std::thread::JoinHandle<()>, timeout: Duration) {
        let deadline = std::time::Instant::now() + timeout;
        while !handle.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(handle.is_finished(), "serve 线程应在 {timeout:?} 内退出");
        handle.join().expect("serve 线程不应 panic");
    }
```

测试一：HTTP 语义往返（ureq 3 需 `http_status_as_error(false)` 的 agent 才能断言非 2xx 状态码）：

```rust
    #[test]
    fn test_serve_roundtrip() {
        run_with_temp_home(|_| {
            let (handle, running, event_rx, port) = spawn_serve();
            // 创建不把非 2xx 当错误的 agent，方便断言所有状态码
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .http_status_as_error(false)
                .build()
                .into();

            // 有效事件 → 204 且 sink 收到；错 token → 401；坏 JSON → 400；
            // 未知 type → 204 不产生事件（recv_timeout 断言超时防排队干扰）；
            // 未知路径 → 404；非 POST → 405（完整断言见 src/dsh/mod.rs）
            // ... 逐项 POST/GET 后 assert_eq!(resp.status(), ...) ...

            // 停止：running=false 后线程应在 ~1 个 RECV_TIMEOUT 内退出
            running.store(false, std::sync::atomic::Ordering::Relaxed);
            join_with_deadline(handle, Duration::from_secs(1));
        });
    }
```

测试二：停滞客户端不无限阻塞停止——**断言边界**：完全停滞且不断开的客户端在
tiny_http API 下无解（socket 无读超时，read 永不返回；且 respond 内部
`EqualReader::drop` 会无超时 drain 剩余 body），完整保证由 Task 6 调用方的
有界等待兜底。本测试锁定「客户端断开后 serve 有界退出」：

```rust
    #[test]
    fn test_serve_stalled_client_unblocks_on_stop() {
        run_with_temp_home(|_| {
            let (handle, running, _event_rx, port) = spawn_serve();

            // 裸 TCP 手写 HTTP 头：有效 Bearer + Content-Length: 2000，发 50 字节后停住
            use std::io::Write as _;
            let mut sock = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            let head = format!(
                "POST /dsh/events HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Authorization: Bearer test-token\r\nContent-Length: 2000\r\n\r\n"
            );
            sock.write_all(head.as_bytes()).unwrap();
            sock.write_all(&vec![b'x'; 50]).unwrap();
            // 等 serve 消费掉已发的 50 字节并阻塞在下一次 read（socket 无读超时）
            std::thread::sleep(Duration::from_millis(100));

            // 停止标志置 false 后客户端断开连接：阻塞的 read 以 EOF/错误返回，
            // serve 放弃该请求（body 非完整 JSON → 400）并回循环头退出
            running.store(false, std::sync::atomic::Ordering::Relaxed);
            drop(sock);
            join_with_deadline(handle, Duration::from_secs(1));
        });
    }
```

测试三：413 护栏（65KB → 413；恰好 64KB 合法载荷 → 204）：

```rust
    #[test]
    fn test_serve_body_size_limit() {
        run_with_temp_home(|_| {
            let (handle, running, _event_rx, port) = spawn_serve();
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .http_status_as_error(false)
                .build()
                .into();

            // title 填充至 body 恰好 MAX_BODY_BYTES 字节的合法 JSON → 非 413、204
            let prefix = r#"{"type":"task-started","session_id":"s1","title":""#;
            let suffix = r#""}"#;
            let pad = MAX_BODY_BYTES as usize - prefix.len() - suffix.len();
            let exact = format!("{prefix}{}{suffix}", "x".repeat(pad));
            assert_eq!(exact.len(), MAX_BODY_BYTES as usize);
            // ... POST exact → assert_ne!(413) + assert_eq!(204) ...

            // 65KB 超限载荷 → 413（Content-Length 预检直接拒）
            // ... POST oversized → assert_eq!(413) ...

            running.store(false, std::sync::atomic::Ordering::Relaxed);
            join_with_deadline(handle, Duration::from_secs(1));
        });
    }
```

**Step 2: 跑测试确认失败** → `cargo test test_serve_roundtrip -- --test-threads=1`（serve 不存在）

**Step 3: 实现**（mod.rs 追加）

```rust
/// 「停止中放弃请求」状态码：serve 循环对其做分离响应（见 [`serve`] 内注释）。
const STOPPED_MIDWAY_STATUS: u16 = 500;

/// 桥服务主循环：绑定 loopback → `on_ready(实际端口)` → 循环收事件交给 sink。
///
/// - `port == 0` 绑随机端口（推荐，避免冲突）
/// - 停止保证覆盖「请求间」检查：空闲/请求间隙时 `running == false` 后最多一个
///   [`RECV_TIMEOUT`] 周期内退出；请求处理中（body 读取/响应收尾）若被停滞
///   客户端阻塞，serve 本层无法有界退出（tiny_http 无 socket 读超时），停止
///   延迟由调用方的有界等待兜底（Task 6 `stop_dsh_bridge_inner`）
/// - body 读取中途发现 `running == false` 会放弃该请求并移交分离线程收尾，
///   对滴流式客户端可尽快让出循环（best-effort，非停止保证的一部分）
/// - 事件处理（节流/台词/分发）在 sink 闭包内，本层只管 HTTP 语义
pub fn serve(
    port: u16,
    token: &str,
    sink: &mut dyn FnMut(event::DshEvent),
    running: &std::sync::atomic::AtomicBool,
    on_ready: &mut dyn FnMut(u16),
) -> Result<(), String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("绑定 127.0.0.1:{port} 失败: {e}"))?;
    let actual = listener
        .local_addr()
        .map_err(|e| format!("获取监听端口失败: {e}"))?
        .port();
    let server = tiny_http::Server::from_listener(listener, None)
        .map_err(|e| format!("启动 HTTP 服务失败: {e}"))?;
    on_ready(actual);
    tracing::info!("dsh 桥监听 127.0.0.1:{actual}");
    loop {
        if !running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }
        match server.recv_timeout(RECV_TIMEOUT) {
            Ok(Some(mut request)) => {
                let status = handle_request(&mut request, token, sink, running);
                if status == STOPPED_MIDWAY_STATUS {
                    // 停止中放弃的请求（body 可能未读完）：respond 内部 drop 请求体
                    // reader 时会同步 drain 剩余字节（EqualReader::drop），停滞客户端
                    // 会让它永久阻塞。移交分离线程收尾，serve 循环立即回循环头退出；
                    // 分离线程在客户端断开或进程退出时自然终结（不参与停止等待）
                    std::thread::spawn(move || {
                        let _ = request.respond(tiny_http::Response::empty(status));
                    });
                } else {
                    let _ = request.respond(tiny_http::Response::empty(status));
                }
            }
            Ok(None) => {} // 超时：回循环头检查 running
            // tiny_http 推 Error 后内部 listener 已死，继续轮询只是僵尸；
            // 返回 Err 让调用方（Task 6）走错误状态 emit
            Err(e) => return Err(format!("dsh 桥 accept 异常: {e}")),
        }
    }
}

/// 处理单条请求，返回响应状态码。
fn handle_request(
    request: &mut tiny_http::Request,
    token: &str,
    sink: &mut dyn FnMut(event::DshEvent),
    running: &std::sync::atomic::AtomicBool,
) -> u16 {
    if request.method() != &tiny_http::Method::Post {
        return 405;
    }
    if request.url() != "/dsh/events" {
        return 404;
    }
    let expected = format!("Bearer {token}");
    let authorized = request
        .headers()
        .iter()
        .any(|h| h.field.equiv("Authorization") && h.value.as_str() == expected);
    if !authorized {
        return 401;
    }
    if request.body_length().is_some_and(|len| len as u64 > MAX_BODY_BYTES) {
        return 413;
    }
    // 读取请求体：累积字节后一次性转 String（按块转换会在 4096 边界损毁多字节
    // 字符）；`take` 不可用于 trait object，手动限制读满 MAX_BODY_BYTES
    let mut body_bytes: Vec<u8> = Vec::new();
    {
        let mut buf = [0u8; 4096];
        let reader = request.as_reader();
        loop {
            match std::io::Read::read(reader, &mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    body_bytes.extend_from_slice(&buf[..n]);
                    if body_bytes.len() as u64 > MAX_BODY_BYTES {
                        return 413;
                    }
                    // 停止中放弃剩余 body（best-effort）：serve 循环对该状态码
                    // 做分离响应后立即退出，滴流式客户端不再占用循环
                    if !running.load(std::sync::atomic::Ordering::Relaxed) {
                        return STOPPED_MIDWAY_STATUS;
                    }
                }
                Err(_) => return 400,
            }
        }
    }
    let body = String::from_utf8_lossy(&body_bytes).into_owned();
    match event::parse_event(&body) {
        Ok(Some(ev)) => {
            sink(ev);
            204
        }
        Ok(None) => {
            tracing::debug!("dsh 桥忽略未知类型事件: {}", body.chars().take(200).collect::<String>());
            204
        }
        Err(e) => {
            tracing::warn!("dsh 桥事件解析失败: {e}");
            400
        }
    }
}
```

> 实现注记：`Read::take` 不可用于 `as_reader()` 返回的 trait object，改手动累积
> 读取；ureq 3 默认把非 2xx 当 `Err`，测试需 `http_status_as_error(false)` agent。

**Step 4: 跑测试确认通过** → `cargo test test_serve_roundtrip -- --test-threads=1`

**Step 5: Commit**

```bash
git add src/dsh/
git commit -m "feat(dsh): tiny_http 桥服务（401/400/404/405/413/204 语义 + recv_timeout 可停）"
```

---

### Task 6: src-tauri 接线——State、管线、commands、setup 挂载

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: 实现代码**（本任务以编译 + clippy 为验收，行为联调在 Task 10）

在语音会话区块（`make_voice_emit` / `start_voice_session_impl` 一带）之后新增分节：

```rust
// ---- dsh 桥（deepseek-harness 任务事件 → 桌宠说话）----

/// dsh 桥状态：共享停止标志 + 线程句柄 + 实际监听端口（RuntimeActual）。
///
/// running/port 指向「当前一代」桥的标志：每次 start 整体替换为全新 Arc（fresh），
/// 服务线程持有克隆。stop 超时分离的旧线程迟到退出时只作用于自己那一代——
/// 不会读到新桥的 true 而复活，也不会把新桥的 running/port 清掉。
struct DshBridgeState {
    /// 当前代停止标志（false = 应退出）；start 时整体替换
    running: Mutex<Arc<AtomicBool>>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// 当前代实际监听端口（0 = 未运行/未就绪）；start 时整体替换
    port: Mutex<Arc<AtomicU16>>,
    /// 最近一次运行的错误（线程 epilogue 写入、start 清空；供前端轮询）
    last_error: Mutex<Option<String>>,
}

impl DshBridgeState {
    fn new() -> Self {
        Self {
            running: Mutex::new(Arc::new(AtomicBool::new(false))),
            handle: Mutex::new(None),
            port: Mutex::new(Arc::new(AtomicU16::new(0))),
            last_error: Mutex::new(None),
        }
    }

    fn is_running(&self) -> bool {
        self.current_running().load(Ordering::Relaxed)
    }

    /// 当前代停止标志（克隆后读取，锁即刻释放）。
    fn current_running(&self) -> Arc<AtomicBool> {
        self.running
            .lock()
            .expect("dsh running lock poisoned")
            .clone()
    }

    /// 给定标志是否仍是「当前代」（线程 epilogue 写共享状态前的守卫：
    /// 分离的旧线程迟到退出时 start 已换新 Arc，此时不写 last_error、不发终态事件）。
    fn is_current_generation(&self, running: &Arc<AtomicBool>) -> bool {
        Arc::ptr_eq(&self.current_running(), running)
    }

    /// 当前实际监听端口（0 = 未运行/未就绪）。
    fn current_port(&self) -> u16 {
        self.port
            .lock()
            .expect("dsh port lock poisoned")
            .load(Ordering::Relaxed)
    }
}

/// `dsh-speak` 事件载荷（气泡台词 + 原始事件）。
#[derive(Clone, Serialize)]
struct DshSpeakPayload {
    text: String,
    event: zapmomo::dsh::event::DshEvent,
}

/// `dsh-bridge-status` 事件载荷。
#[derive(Clone, Serialize)]
struct DshBridgeStatusPayload {
    running: bool,
    port: Option<u16>,
    error: Option<String>,
}

/// dsh 事件处理管线：节流 → 模板台词 → `dsh-speak` emit。
/// （阶段 2 在 emit 后追加 TTS 播报与对话记录落盘。）
fn handle_dsh_event(app: &AppHandle, throttle: &zapmomo::dsh::EventThrottle, event: zapmomo::dsh::event::DshEvent) {
    if !throttle.allow(&event) {
        tracing::debug!(
            "dsh 事件被节流丢弃: kind={} session={}",
            event.kind(),
            event.session_id()
        );
        return;
    }
    let text = zapmomo::dsh::lines::pick_line(&event, zapmomo::dsh::lines::next_roll());
    tracing::info!("dsh 事件播报: kind={} text={text}", event.kind());
    let _ = app.emit("dsh-speak", DshSpeakPayload { text, event });
}

/// 启动 dsh 桥：解析配置 → spawn 服务线程（绑 loopback、写发现文件、事件走管线）。
fn start_dsh_bridge_impl(app: AppHandle, state: &DshBridgeState) -> Result<(), String> {
    if state.is_running() {
        return Err("dsh 桥已在运行".to_string());
    }
    let settings = zapmomo::config::settings::load_settings()?;
    let cfg = zapmomo::dsh::config::resolve(settings.as_ref().and_then(|s| s.dsh.as_ref()));
    if !cfg.enabled {
        return Err("dsh 桥未启用".to_string());
    }
    // 清陈旧发现文件（上次退出未清理的残留）
    zapmomo::dsh::remove_discovery();

    // fresh Arc：每次启动换全新一代标志（线程持有克隆）——stop 超时分离的旧线程
    // 迟到退出只作用于自己那一代，不会复活（旧 running 恒 false）也不会污染新桥
    let running = Arc::new(AtomicBool::new(true));
    *state.running.lock().expect("dsh running lock poisoned") = running.clone();
    let port_flag = Arc::new(AtomicU16::new(0));
    *state.port.lock().expect("dsh port lock poisoned") = port_flag.clone();
    *state
        .last_error
        .lock()
        .expect("dsh last_error lock poisoned") = None;
    let thread_app = app;
    let handle = std::thread::spawn(move || {
        tracing::info!("dsh bridge thread started");
        let token = zapmomo::dsh::generate_token();
        let token_for_file = token.clone();
        let port_for_ready = port_flag.clone();
        let app_for_ready = thread_app.clone();
        let mut on_ready = move |port: u16| {
            port_for_ready.store(port, Ordering::Relaxed);
            if let Err(e) = zapmomo::dsh::write_discovery(&zapmomo::dsh::DiscoveryInfo {
                port,
                token: token_for_file.clone(),
            }) {
                tracing::warn!("dsh 桥发现文件写入失败: {e}");
            }
            let _ = app_for_ready.emit(
                "dsh-bridge-status",
                DshBridgeStatusPayload {
                    running: true,
                    port: Some(port),
                    error: None,
                },
            );
        };
        let throttle = zapmomo::dsh::EventThrottle::new(std::time::Duration::from_secs(3));
        let app_for_sink = thread_app.clone();
        let mut sink = move |event: zapmomo::dsh::event::DshEvent| {
            handle_dsh_event(&app_for_sink, &throttle, event);
        };
        let result = zapmomo::dsh::serve(cfg.port, &token, &mut sink, &running, &mut on_ready);
        port_flag.store(0, Ordering::Relaxed);
        // token 条件清理：本线程若是迟到的分离线程，发现文件可能已属于重启后的
        // 新桥——只有 token 一致（仍是自己的文件）才删
        zapmomo::dsh::remove_discovery_if_token(&token);
        running.store(false, Ordering::Relaxed);
        match &result {
            Ok(()) => tracing::info!("dsh bridge thread finished (clean)"),
            Err(e) => tracing::error!("dsh bridge thread finished with error: {e}"),
        }
        // 本线程仍是当前代才写 last_error / 发终态事件（分离旧线程迟到退出
        // 不污染新桥的轮询状态与前端事件）
        let bridge_state = thread_app.state::<DshBridgeState>();
        if bridge_state.is_current_generation(&running) {
            let err = result.err();
            *bridge_state
                .last_error
                .lock()
                .expect("dsh last_error lock poisoned") = err.clone();
            let _ = thread_app.emit(
                "dsh-bridge-status",
                DshBridgeStatusPayload {
                    running: false,
                    port: None,
                    error: err,
                },
            );
        }
    });
    *state
        .handle
        .lock()
        .expect("dsh bridge handle lock poisoned") = Some(handle);
    Ok(())
}

/// 停止 dsh 桥：置停止标志后有界等待线程退出。
///
/// serve 的停止保证覆盖「请求间」检查；停滞客户端可能把线程卡在单次请求的
/// body 读取/响应抽干里（tiny_http 无 socket 读超时），此时超时后**分离线程**
/// （不 join，不阻塞调用方；线程随客户端断开或进程退出自然终结），running
/// 状态位仍保持一致。分离线程迟到退出只作用于自己那一代标志（fresh Arc）；
/// 发现文件若因此未由 epilogue 清理，下次启动会清陈旧残留。
fn stop_dsh_bridge_inner(state: &DshBridgeState) -> Result<(), String> {
    if !state.is_running() {
        return Err("dsh 桥未在运行".to_string());
    }
    state.current_running().store(false, Ordering::Relaxed);
    let handle = state
        .handle
        .lock()
        .expect("dsh bridge handle lock poisoned")
        .take();
    if let Some(handle) = handle {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !handle.is_finished() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if handle.is_finished() {
            let _ = handle.join();
        } else {
            tracing::warn!("dsh 桥线程未在 2s 内退出（可能被停滞客户端阻塞），已分离");
            // JoinHandle 无 Drop impl，forget 与 drop 同为分离语义；显式 forget 表达意图
            std::mem::forget(handle);
        }
    }
    Ok(())
}
```

commands（同分节内追加）：

```rust
/// GUI 展示用的 dsh 桥配置信息。
#[derive(Serialize)]
struct DshConfigInfo {
    enabled: bool,
    port: u16,
    voice_enabled: bool,
    record_to_history: bool,
    running: bool,
    /// 实际监听端口（RuntimeActual；None = 未就绪）
    actual_port: Option<u16>,
    discovery_path: String,
}

#[tauri::command]
fn get_dsh_config(state: State<'_, DshBridgeState>) -> Result<DshConfigInfo, String> {
    let settings = zapmomo::config::settings::load_settings()?;
    let cfg = zapmomo::dsh::config::resolve(settings.as_ref().and_then(|s| s.dsh.as_ref()));
    let actual = state.current_port();
    Ok(DshConfigInfo {
        enabled: cfg.enabled,
        port: cfg.port,
        voice_enabled: cfg.voice_enabled,
        record_to_history: cfg.record_to_history,
        running: state.is_running(),
        actual_port: (actual != 0).then_some(actual),
        discovery_path: zapmomo::dsh::discovery_file().display().to_string(),
    })
}

#[tauri::command]
fn set_dsh_enabled(
    app: AppHandle,
    state: State<'_, DshBridgeState>,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = zapmomo::config::settings::load_settings()?.unwrap_or_default();
    settings.dsh.get_or_insert_with(Default::default).enabled = Some(enabled);
    zapmomo::config::settings::save_settings(&settings)?;
    if enabled {
        if state.is_running() {
            // 幂等：目标态已达成直接成功（避免重复开关弹错误提示）
            Ok(())
        } else {
            start_dsh_bridge_impl(app, state.inner())
        }
    } else if state.is_running() {
        stop_dsh_bridge_inner(state.inner())
    } else {
        Ok(())
    }
}

/// `set_dsh_params` 载荷：可调整项（缺省不修改）。
#[derive(Debug, Clone, Default, Deserialize)]
struct DshParamsPatch {
    voice_enabled: Option<bool>,
    record_to_history: Option<bool>,
    port: Option<u16>,
}

#[tauri::command]
fn set_dsh_params(
    app: AppHandle,
    state: State<'_, DshBridgeState>,
    params: DshParamsPatch,
) -> Result<(), String> {
    if let Some(p) = params.port
        && p != 0
        && p < 1024
    {
        return Err(format!("端口需在 1024~65535 或 0（随机），当前 {p}"));
    }
    let mut settings = zapmomo::config::settings::load_settings()?.unwrap_or_default();
    // 端口与当前配置相同（未配置 ≡ 0 随机）则无需重启桥
    let port_changed = params
        .port
        .is_some_and(|p| settings.dsh.as_ref().and_then(|d| d.port).unwrap_or(0) != p);
    let dsh = settings.dsh.get_or_insert_with(Default::default);
    if let Some(v) = params.voice_enabled {
        dsh.voice_enabled = Some(v);
    }
    if let Some(v) = params.record_to_history {
        dsh.record_to_history = Some(v);
    }
    if let Some(v) = params.port {
        dsh.port = Some(v);
    }
    zapmomo::config::settings::save_settings(&settings)?;
    // 端口变化需重启桥生效；voice/record 项在事件时实时读取
    if port_changed && state.is_running() {
        stop_dsh_bridge_inner(state.inner())?;
        start_dsh_bridge_impl(app, state.inner())?;
    }
    Ok(())
}

#[tauri::command]
fn get_dsh_bridge_status(state: State<'_, DshBridgeState>) -> DshBridgeStatusPayload {
    let port = state.current_port();
    DshBridgeStatusPayload {
        running: state.is_running(),
        port: (port != 0).then_some(port),
        error: state
            .last_error
            .lock()
            .expect("dsh last_error lock poisoned")
            .clone(),
    }
}

/// 测试播报：灌一条假事件进管线（设置页按钮全链路验收，不用 curl）。
#[tauri::command]
fn test_dsh_announce(app: AppHandle) -> Result<(), String> {
    // 独立零窗口节流器：测试不受 3s 节流限制
    let throttle = zapmomo::dsh::EventThrottle::new(std::time::Duration::ZERO);
    handle_dsh_event(
        &app,
        &throttle,
        zapmomo::dsh::event::DshEvent::TaskFinished {
            session_id: "zapmomo-test".to_string(),
            title: Some("桌宠测试播报".to_string()),
            reason: Some("completed".to_string()),
        },
    );
    Ok(())
}
```

三处注册：

1. `.manage(...)` 链（`VoiceSessionState::new()` 之后）：`.manage(DshBridgeState::new())`
2. `generate_handler![...]` 列表（`is_voice_session_running,` 之后）：
   `get_dsh_config, set_dsh_enabled, set_dsh_params, get_dsh_bridge_status, test_dsh_announce,`
3. `setup()` 内 KWS 自动监听块（`if !voice_auto_started && ...` 结束的 `}` 之后、companion 建窗之前）：

```rust
            // 启动 dsh 桥（若启用）：loopback HTTP 接收 deepseek-harness 插件推送的
            // 任务事件，桌宠以气泡+语音播报。失败静默降级（不影响主流程）。
            if zapmomo::dsh::config::resolve(loaded.as_ref().and_then(|s| s.dsh.as_ref())).enabled {
                let handle = app.handle().clone();
                let state = app.state::<DshBridgeState>();
                if let Err(e) = start_dsh_bridge_impl(handle, state.inner()) {
                    tracing::warn!("自动启动 dsh 桥失败: {e}");
                }
            }
```

顶部 use 补 `std::sync::atomic::AtomicU16`（现有 atomic import 行内追加）。

> **审查修复补充（已实施）**：
> 1. 根 crate 新增 `dsh::remove_discovery_if_token(&str)`（含单测，见 `src/dsh/mod.rs`）——
>    桥线程 epilogue 的发现文件清理由无条件 `remove_discovery()` 改为 token 条件式，
>    防止 stop 超时分离的旧线程迟到退出时误删新桥的发现文件；
> 2. `DshBridgeState` 的 running/port 每次启动整体替换为全新 Arc（fresh），并新增
>    `last_error` 与 `is_current_generation`（`Arc::ptr_eq` 守卫：分离旧线程迟到退出
>    不写 last_error、不发终态事件），`stop_dsh_bridge_inner` 经当前代 Arc 置停止标志；
> 3. `set_dsh_enabled` 开启且已在运行时幂等返回成功；`set_dsh_params` 端口与当前
>    配置相同则跳过重启；退出/重启路径（`handle_menu` quit/restart 与 `quit_app`/
>    `restart_app`）先 `remove_discovery()` 再退出，防死端口文件残留。
> 上述代码块已同步为最终版本。

**Step 2: 编译与 Lint**

Run: `cargo check -p zapmomo-app && cargo clippy -p zapmomo-app -- -D warnings`
Expected: 通过（Linux/webkit 依赖缺失环境可跳过 clippy，仅 `cargo check`；本机 macOS 可全跑）

**Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(dsh): Tauri 接线——桥线程状态、事件管线、5 个 command 与启动自启"
```

---

### Task 7: 前端类型与 tauri.ts 封装

**Files:**
- Modify: `src-tauri/frontend/src/types/tauri.ts`（文件末尾追加）
- Modify: `src-tauri/frontend/src/lib/tauri.ts`

**实现**（types/tauri.ts 末尾）：

```ts
/** dsh 任务事件（后端 DshEvent 序列化；type 为 kebab-case 判别字段） */
export interface DshEventInfo {
  type: "task-started" | "task-finished" | "task-failed" | "task-interrupted";
  session_id: string;
  title?: string | null;
  reason?: string | null;
  detail?: string | null;
}

/** `dsh-speak` 事件载荷（气泡台词 + 原始事件） */
export interface DshSpeakPayload {
  text: string;
  event: DshEventInfo;
}

/** `dsh-bridge-status` 事件载荷 / `get_dsh_bridge_status` 返回 */
export interface DshBridgeStatus {
  running: boolean;
  port: number | null;
  error: string | null;
}

/** `get_dsh_config` 返回 */
export interface DshConfigInfo {
  enabled: boolean;
  port: number;
  voice_enabled: boolean;
  record_to_history: boolean;
  running: boolean;
  actual_port: number | null;
  discovery_path: string;
}

/** `set_dsh_params` 载荷（snake_case 直传，缺省项不修改） */
export interface DshParamsPatch {
  voice_enabled?: boolean;
  record_to_history?: boolean;
  port?: number;
}
```

tauri.ts——import type 列表追加 `DshBridgeStatus, DshConfigInfo, DshParamsPatch, DshSpeakPayload`；
api 对象「对话记录」段之后追加：

```ts
  // ---- dsh 桥（deepseek-harness 任务事件 → 桌宠说话）----
  getDshConfig: () => invoke<DshConfigInfo>("get_dsh_config"),
  setDshEnabled: (args: { enabled: boolean }) => invoke<void>("set_dsh_enabled", args),
  setDshParams: (args: { params: DshParamsPatch }) => invoke<void>("set_dsh_params", args),
  getDshBridgeStatus: () => invoke<DshBridgeStatus>("get_dsh_bridge_status"),
  testDshAnnounce: () => invoke<void>("test_dsh_announce"),
```

文件末尾（`onVoiceSessionStopped` 之后）追加：

```ts
// ---- dsh 桥事件 ----

export function onDshSpeak(handler: (payload: DshSpeakPayload) => void): Promise<UnlistenFn> {
  return listen<DshSpeakPayload>("dsh-speak", (e) => handler(e.payload));
}

export function onDshBridgeStatus(handler: (payload: DshBridgeStatus) => void): Promise<UnlistenFn> {
  return listen<DshBridgeStatus>("dsh-bridge-status", (e) => handler(e.payload));
}
```

**验收:** `pnpm -C src-tauri/frontend exec tsc -b`（类型检查通过）

**Commit:**

```bash
git add src-tauri/frontend/src/types/tauri.ts src-tauri/frontend/src/lib/tauri.ts
git commit -m "feat(dsh): 前端类型与 command/事件订阅封装"
```

---

### Task 8: EventBubble 气泡组件（TDD）

**Files:**
- Create: `src-tauri/frontend/src/components/companion/EventBubble.tsx`
- Test: `src-tauri/frontend/src/components/companion/EventBubble.test.tsx`

**Step 1: 写失败测试**（EventBubble.test.tsx）

```tsx
import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EventBubble } from "./EventBubble";

const { listenMock, eventHandlers } = vi.hoisted(() => {
  const handlers: Record<string, (payload: unknown) => void> = {};
  return {
    listenMock: vi.fn((event: string, cb: (e: { payload: unknown }) => void) => {
      handlers[event] = (payload) => cb({ payload });
      return Promise.resolve(() => {});
    }),
    eventHandlers: handlers,
  };
});

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

function emitSpeak(text: string) {
  act(() => {
    eventHandlers["dsh-speak"]?.({ text, event: { type: "task-finished", session_id: "s" } });
  });
}

async function renderReady() {
  render(<EventBubble />);
  await act(async () => {
    await Promise.resolve();
  });
}

describe("EventBubble", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-21T10:00:00Z"));
    vi.clearAllMocks();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("订阅 dsh-speak，收到事件渲染气泡，8 秒后自动消失", async () => {
    await renderReady();
    expect(listenMock).toHaveBeenCalledWith("dsh-speak", expect.any(Function));
    emitSpeak("任务搞定啦！");
    expect(screen.getByText("任务搞定啦！")).toBeTruthy();
    act(() => {
      vi.advanceTimersByTime(8500);
    });
    expect(screen.queryByText("任务搞定啦！")).toBeNull();
  });

  it("同时最多显示 2 条：第 3 条出现时最旧的让位", async () => {
    await renderReady();
    emitSpeak("一");
    emitSpeak("二");
    emitSpeak("三");
    expect(screen.queryByText("一")).toBeNull();
    expect(screen.getByText("二")).toBeTruthy();
    expect(screen.getByText("三")).toBeTruthy();
  });

  it("队列上限 3：第 4 条到达时最旧的出队", async () => {
    await renderReady();
    emitSpeak("一");
    emitSpeak("二");
    emitSpeak("三");
    emitSpeak("四");
    // 队列内只剩 二三四，可见的是最新两条
    expect(screen.queryByText("一")).toBeNull();
    expect(screen.getByText("三")).toBeTruthy();
    expect(screen.getByText("四")).toBeTruthy();
  });
});
```

**Step 2: 跑测试确认失败**

Run: `pnpm -C src-tauri/frontend exec vitest run src/components/companion/EventBubble.test.tsx`
Expected: FAIL（组件不存在）

**Step 3: 实现组件**

```tsx
import { useEffect, useRef, useState } from "react";
import { onDshSpeak } from "@/lib/tauri";
import type { DshSpeakPayload } from "@/types/tauri";

/** 气泡自动消失时间 */
const VISIBLE_MS = 8000;
/** 保留队列上限（超出丢最旧） */
const MAX_QUEUE = 3;
/** 同时展示上限（更旧的让位） */
const MAX_BUBBLES = 2;
/** 过期裁剪轮询周期（淡出动画的驱动源） */
const PRUNE_INTERVAL_MS = 500;

interface Bubble {
  id: number;
  text: string;
  /** 到期时间戳（Date.now() + VISIBLE_MS） */
  until: number;
}

/**
 * 桌宠事件气泡：订阅 `dsh-speak`，在角色上方展示模板台词。
 *
 * - 队列上限 3、同时最多 2 条、每条 8s 自动淡出（500ms 轮询裁剪，fake timers 可测）
 * - 最多两行截断（`line-clamp-2`；全文有对话记录兜底）
 * - `pointer-events-none`：不挡窗口拖动/右键/滚轮
 */
export function EventBubble() {
  const [bubbles, setBubbles] = useState<Bubble[]>([]);
  const nextIdRef = useRef(1);

  useEffect(() => {
    const unlisten = onDshSpeak(({ text }: DshSpeakPayload) => {
      const id = nextIdRef.current++;
      setBubbles((prev) =>
        [...prev, { id, text, until: Date.now() + VISIBLE_MS }].slice(-MAX_QUEUE),
      );
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  // 定期裁剪过期气泡（interval 依赖 bubbles：空列表不挂定时器）
  useEffect(() => {
    if (bubbles.length === 0) return;
    const timer = setInterval(() => {
      const now = Date.now();
      setBubbles((prev) =>
        prev.some((b) => b.until <= now) ? prev.filter((b) => b.until > now) : prev,
      );
    }, PRUNE_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [bubbles]);

  const visible = bubbles.slice(-MAX_BUBBLES);
  const now = Date.now();

  return (
    <div
      data-testid="dsh-bubbles"
      className="pointer-events-none absolute inset-x-0 top-6 z-10 flex flex-col items-center gap-1 px-4"
    >
      {visible.map((b) => (
        <div
          key={b.id}
          className="max-w-[240px] rounded-2xl bg-black/70 px-3 py-1.5 text-center text-xs leading-relaxed text-white shadow-lg backdrop-blur-sm"
          style={{ opacity: b.until - now < 600 ? 0 : 1, transition: "opacity 500ms" }}
        >
          <span className="line-clamp-2">{b.text}</span>
        </div>
      ))}
    </div>
  );
}
```

**Step 4: 跑测试确认通过** → 同 Step 2 命令，Expected: 3 PASS

**Step 5: Commit**

```bash
git add src-tauri/frontend/src/components/companion/
git commit -m "feat(dsh): 桌宠事件气泡组件（队列 3 / 同屏 2 / 8s 淡出）"
```

---

### Task 9: motion 联动 + CompanionRoot 集成

**Files:**
- Create: `src-tauri/frontend/src/lib/dshMotion.ts`
- Test: `src-tauri/frontend/src/lib/dshMotion.test.ts`
- Modify: `src-tauri/frontend/src/components/live2d/Live2dStage.tsx`（新增 `onModelLoaded` prop）
- Modify: `src-tauri/frontend/src/components/CompanionRoot.tsx`

**Step 1: 写 dshMotion 失败测试**（dshMotion.test.ts）

```ts
import { describe, expect, it } from "vitest";
import { pickMotionGroup } from "./dshMotion";

describe("pickMotionGroup", () => {
  it("按事件类型提示词匹配组名（大小写不敏感子串）", () => {
    expect(pickMotionGroup(["Idle", "TapBody", "FlickHead"], "task-started")).toBe("TapBody");
    expect(pickMotionGroup(["Idle", "Happy", "Tap"], "task-finished")).toBe("Happy");
    expect(pickMotionGroup(["idle", "Sad"], "task-failed")).toBe("Sad");
  });

  it("未命中返回 null（调用方静默跳过）", () => {
    expect(pickMotionGroup(["Idle"], "task-finished")).toBeNull();
    expect(pickMotionGroup([], "task-started")).toBeNull();
  });
});
```

**Step 2: 实现 dshMotion.ts**

```ts
import type { DshEventType } from "@/types/tauri";

/** 事件类型 → motion 组名提示词（大小写不敏感子串匹配） */
const MOTION_HINTS: Record<DshEventType, string[]> = {
  "task-started": ["tap", "flick", "greet", "hello"],
  "task-finished": ["happy", "smile", "joy", "win", "dance"],
  "task-failed": ["sad", "cry", "angry", "shock"],
  "task-interrupted": ["idle", "surprise", "think"],
};

/**
 * 从模型可用 motion 组里挑一个匹配的（第一个命中提示词的组）。
 * 模型组名千差万别，匹配不到返回 null（调用方静默跳过）。
 */
export function pickMotionGroup(groups: string[], type: DshEventType): string | null {
  const hints = MOTION_HINTS[type] ?? [];
  for (const hint of hints) {
    const idx = groups.findIndex((g) => g.toLowerCase().includes(hint));
    if (idx >= 0) return groups[idx];
  }
  return null;
}
```

**Step 3: Live2dStage 增加 `onModelLoaded` prop**

Props 增加并实现（仿现有 `onModelMetrics` 的 ref 转发模式）：

```ts
  /** 模型加载成功（或清屏置 null）时回调，供上层持有模型句柄触发动作。 */
  onModelLoaded?: (model: Live2DModel | null) => void;
```

组件内（`onModelReadyRef` 旁）：

```ts
  const onModelLoadedRef = useRef(onModelLoaded);
  onModelLoadedRef.current = onModelLoaded;
```

模型加载 effect 中三处调用：
- `modelUrl` 为 null 的清屏分支：`modelRef.current?.destroy(); modelRef.current = null;` 后加 `onModelLoadedRef.current?.(null);`
- 加载成功 `modelRef.current = model;` 后加 `onModelLoadedRef.current?.(model);`
- `cancelled` 早退分支不动（未注册）

**Step 4: CompanionRoot 集成**

import 增加：

```ts
import type { Live2DModel } from "pixi-live2d-display/cubism4";
import { EventBubble } from "@/components/companion/EventBubble";
import { pickMotionGroup } from "@/lib/dshMotion";
import { onDshSpeak } from "@/lib/tauri";
```

组件内（`locked` state 之后）：

```tsx
  // Live2D 模型句柄：dsh 事件触发动作用（模型缺对应组时静默跳过）。
  const modelRef = useRef<Live2DModel | null>(null);

  // dsh 任务事件：气泡由 EventBubble 渲染，这里联动触发模型动作。
  useEffect(() => {
    const unlisten = onDshSpeak(({ event }) => {
      const model = modelRef.current;
      if (!model) return;
      const groups = Object.keys(
        (model.internalModel.motionManager.definitions ?? {}) as Record<string, unknown>,
      );
      const group = pickMotionGroup(groups, event.type);
      if (!group) return;
      // FORCE 优先级（3）：打断 idle/在播动作，同 previewManager 的 startMotion 语义
      void model.internalModel.motionManager.startMotion(group, 0, 3);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);
```

`<Live2dStage ... onModelMetrics={handleModelMetrics} />` 增加：

```tsx
          onModelLoaded={(m) => {
            modelRef.current = m;
          }}
```

JSX 中 `<VoiceStatusDot` 区块之前插入：

```tsx
      {/* dsh 任务事件气泡（pointer-events-none，不挡拖动/右键） */}
      <EventBubble />
```

**Step 5: 跑测试**

Run: `pnpm -C src-tauri/frontend test:run && pnpm -C src-tauri/frontend exec tsc -b`
Expected: 全 PASS（含既有 CompanionRoot.test.tsx——其 listen mock 兼容新增订阅；若因新增订阅断言数量失败，按 mock 实际调用更新断言，不删既有用例）

**Step 6: Commit**

```bash
git add src-tauri/frontend/src/lib/dshMotion.ts src-tauri/frontend/src/lib/dshMotion.test.ts \
        src-tauri/frontend/src/components/live2d/Live2dStage.tsx src-tauri/frontend/src/components/CompanionRoot.tsx \
        src-tauri/frontend/src/components/CompanionRoot.test.tsx
git commit -m "feat(dsh): 桌宠气泡接入 CompanionRoot，事件联动 Live2D motion（未命中静默跳过）"
```

---

### Task 10: 阶段 1 验收

**Step 1: 全量检查**

```bash
cargo fmt --all
cargo fmt --check && cargo clippy -- -D warnings && cargo test -- --test-threads=1
cargo clippy -p zapmomo-app -- -D warnings
pnpm -C src-tauri/frontend test:run && pnpm -C src-tauri/frontend check
```

Expected: 全绿（clippy 报错则修复后重跑）

**Step 2: 手测（curl 全链路）**

```bash
# 1. 启动 app
pnpm tauri dev
# 2. 读发现文件（另开终端）
cat ~/.zapmomo/runtime/dsh-bridge.json
# 3. 有效事件 → 桌宠气泡出现并 8s 消失
PORT=$(jq -r .port ~/.zapmomo/runtime/dsh-bridge.json)
TOKEN=$(jq -r .token ~/.zapmomo/runtime/dsh-bridge.json)
curl -s -o /dev/null -w "%{http_code}\n" -X POST "http://127.0.0.1:$PORT/dsh/events" \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"type":"task-finished","session_id":"s1","title":"修复登录超时"}'
# 期望 204，桌宠出现「「修复登录超时」搞定啦！」气泡
# 4. 错 token → 401，无气泡
curl -s -o /dev/null -w "%{http_code}\n" -X POST "http://127.0.0.1:$PORT/dsh/events" \
  -H "Authorization: Bearer wrong" -d '{"type":"task-finished","session_id":"s1"}'
# 5. 3 秒内重复同事件 → 无第二条气泡（节流）
# 6. 退出 app → 发现文件被删（ls ~/.zapmomo/runtime/）
```

**Step 3: Commit（如有格式修复）**

```bash
git add -A && git commit -m "style(dsh): 阶段1验收 fmt/clippy 修复"
```

---

## 阶段 2：语音播报 + 落盘 + 设置页

### Task 11: `src/dsh/announce.rs`——播报器（TDD）

**Files:**
- Create: `src/dsh/announce.rs`
- Modify: `src/dsh/mod.rs`（声明 `pub mod announce;`）

**Step 1: 写失败测试**（announce.rs 底部）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_announce_plays_via_injected_closures() {
        let (played_tx, played_rx) = mpsc::channel::<(Vec<f32>, u32)>();
        let (synth_tx, _synth_rx) = mpsc::channel::<String>();
        let synth_tx = std::sync::Mutex::new(synth_tx);
        let announcer = Announcer::with(
            move |text| {
                synth_tx.lock().unwrap().send(text.to_string()).unwrap();
                Ok(vec![0.1, 0.2])
            },
            move |samples, rate| {
                played_tx.send((samples, rate)).unwrap();
            },
            24000,
        );
        assert!(announcer.announce("你好"));
        assert_eq!(_synth_rx.recv_timeout(Duration::from_secs(5)).unwrap(), "你好");
        let (samples, rate) = played_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(samples, vec![0.1, 0.2]);
        assert_eq!(rate, 24000);
    }

    #[test]
    fn test_queue_cap_one_overwrites_drops_excess() {
        // 播放闭包阻塞在闭锁上，制造「正在播报」
        let gate = std::sync::Arc::new(std::sync::Mutex::new(()));
        let gate2 = gate.clone();
        let entered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let entered2 = entered.clone();
        let announcer = Announcer::with(
            |_| Ok(vec![0.0]),
            move |_, _| {
                entered2.store(true, std::sync::atomic::Ordering::SeqCst);
                let _g = gate2.lock().unwrap(); // 持锁 = 播放中
            },
            24000,
        );
        assert!(announcer.announce("第一句"), "空闲时应受理");
        // 等播放闭包进入
        while !entered.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
        // 队列容量 1：第二条进队，第三条被丢弃
        assert!(announcer.announce("第二句"), "队列空位应受理");
        assert!(!announcer.announce("第三句"), "队列满应丢弃");
        // 放行播放，让 worker 正常消费
        drop(gate.lock().unwrap());
    }

    #[test]
    fn test_synth_failure_skips_silently() {
        let (played_tx, played_rx) = mpsc::channel::<(Vec<f32>, u32)>();
        let announcer = Announcer::with(
            |_| Err("模型未就绪".to_string()),
            move |samples, rate| {
                played_tx.send((samples, rate)).unwrap();
            },
            24000,
        );
        assert!(announcer.announce("会失败"));
        std::thread::sleep(Duration::from_millis(200));
        assert!(played_rx.try_recv().is_err(), "合成失败不应播放");
        // worker 存活：下一条正常受理
        assert!(announcer.announce("再来"));
    }
}
```

**Step 2: 跑测试确认失败** → `cargo test dsh::announce -- --test-threads=1`

**Step 3: 实现**

```rust
/// dsh 事件语音播报：独立 worker 线程合成 + rodio 播放。
///
/// - 与 voice 会话的互斥由调用方（管线）判断「voice 会话是否运行」决定是否调用
/// - 自身防重叠：worker 串行播放；排队容量 1（`SyncSender::try_send`），溢出丢弃——
///   气泡已传达信息，语音只是增强
/// - 合成/播放函数可注入（测试无音频设备依赖）
use crate::tts;
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::Mutex;
use std::time::Duration;

pub struct Announcer {
    tx: SyncSender<String>,
    _handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Announcer {
    /// 注入合成/播放函数的构造（测试用）。
    ///
    /// `synth(text) -> (PCM 样本)`，`play(samples, sample_rate)` 阻塞到播完。
    pub fn with(
        synth: impl Fn(&str) -> Result<Vec<f32>, String> + Send + 'static,
        play: impl FnMut(Vec<f32>, u32) + Send + 'static,
        sample_rate: u32,
    ) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<String>(1);
        let handle = std::thread::spawn(move || run_worker(rx, synth, play, sample_rate));
        Self {
            tx,
            _handle: Mutex::new(Some(handle)),
        }
    }

    /// 生产构造：sherpa TTS 合成 + rodio Speaker 播放（默认音色/语速）。
    /// TTS 模型未就绪返回 Err（调用方降级为只出气泡；下次事件会重试）。
    pub fn try_new() -> Result<Self, String> {
        let settings = crate::config::settings::load_settings()?;
        let tts_settings = settings.as_ref().and_then(|s| s.tts.clone());
        let cfg = tts::config::resolve(tts_settings.as_ref(), None)?;
        let files = [
            &cfg.encoder,
            &cfg.decoder,
            &cfg.vocoder,
            &cfg.tokens,
            &cfg.lexicon,
        ];
        if let Some(missing) = files.iter().find(|p| !p.is_file()) {
            return Err(format!("TTS 模型未就绪: {}", missing.display()));
        }
        if !cfg.data_dir.is_dir() {
            return Err(format!("TTS 数据目录缺失: {}", cfg.data_dir.display()));
        }
        let (ref_wav, ref_text) = tts::voice::resolve_reference(&cfg, None, None, None)?;
        let engine = tts::TtsEngine::new(cfg.clone())?;
        let sample_rate = engine.sample_rate() as u32;
        let speed = cfg.speed;
        Ok(Self::with(
            move |text| engine.synthesize(text, speed, &ref_wav, &ref_text),
            move |samples, rate| {
                if let Ok(mut speaker) = tts_speaker() {
                    use crate::voice::player::AudioPlayer;
                    speaker.play(samples, rate);
                    // 阻塞到播完（worker 串行语义）
                    while !speaker.drained() {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            },
            sample_rate,
        ))
    }

    /// 请求播报一条文本；正在播/队列满则本条丢弃（返回 false）。
    pub fn announce(&self, text: &str) -> bool {
        self.tx.try_send(text.to_string()).is_ok()
    }
}

/// 每次播报新建 Speaker（独立 OutputStream，与 voice 会话互斥后不会同时出声）。
fn tts_speaker() -> Result<crate::voice::player::Speaker, String> {
    crate::voice::player::Speaker::try_new()
}

fn run_worker(
    rx: Receiver<String>,
    synth: impl Fn(&str) -> Result<Vec<f32>, String>,
    mut play: impl FnMut(Vec<f32>, u32),
    sample_rate: u32,
) {
    while let Ok(text) = rx.recv() {
        match synth(&text) {
            Ok(samples) => play(samples, sample_rate),
            Err(e) => {
                tracing::warn!("dsh 播报合成失败（跳过语音，气泡不受影响）: {e}");
            }
        }
    }
}
```

> 注：`TtsEngine` 是否 `Send + 'static`（含 sherpa-onnx 裸指针）需编译验证；若不满足，
> 把 `TtsEngine::new` 与 `synthesize` 都挪进 `play` 之前的独立线程或改用
> `TtsEngine` 每次 announce 临时构建（事件频率低，成本可接受）。以编译器为准调整，
> 保持「合成失败 → warn 跳过」语义。

**Step 4: 跑测试确认通过**

**Step 5: Commit**

```bash
git add src/dsh/
git commit -m "feat(dsh): 语音播报器——注入式合成/播放、队列容量 1、失败静默跳过"
```

---

### Task 12: 管线接入语音与落盘

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: 改 `DshBridgeState` 与管线**

`DshBridgeState` 增加字段：

```rust
    /// 懒构建的播报器（TTS 模型未就绪时为 None，下次事件重试构建）
    announcer: Mutex<Option<std::sync::Arc<zapmomo::dsh::announce::Announcer>>>,
```

`new()` 内 `announcer: Mutex::new(None),`。

分节内追加：

```rust
/// 取（或懒构建）播报器：TTS 未就绪返回 None（只出气泡），不缓存失败。
fn dsh_announcer(state: &DshBridgeState) -> Option<std::sync::Arc<zapmomo::dsh::announce::Announcer>> {
    let mut slot = state
        .announcer
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(a) = slot.as_ref() {
        return Some(a.clone());
    }
    match zapmomo::dsh::announce::Announcer::try_new() {
        Ok(a) => {
            let a = std::sync::Arc::new(a);
            *slot = Some(a.clone());
            Some(a)
        }
        Err(e) => {
            tracing::debug!("dsh 播报器不可用（只出气泡）: {e}");
            None
        }
    }
}
```

`handle_dsh_event` 的 emit 之后追加（事件时实时读设置，开关即时生效）：

```rust
    let settings = zapmomo::config::settings::load_settings().unwrap_or_default();
    let cfg = zapmomo::dsh::config::resolve(settings.as_ref().and_then(|s| s.dsh.as_ref()));
    // 语音播报：voice 会话运行中不出声（不打断对话）；TTS 未就绪只出气泡
    if cfg.voice_enabled
        && !app.state::<VoiceSessionState>().is_running()
        && let Some(announcer) = dsh_announcer(&app.state::<DshBridgeState>())
    {
        announcer.announce(&text);
    }
    // 落盘到对话记录（与语音会话同库，前端「对话记录」页可见）
    if cfg.record_to_history {
        records::append_record(records::ConversationRecord {
            role: records::RecordRole::Assistant,
            text: text.clone(),
            at: iso_timestamp_now(),
        });
    }
```

> 注意：`text` 在 emit 构造时被 move 的问题——把 `DshSpeakPayload { text, event }` 改为
> `DshSpeakPayload { text: text.clone(), event }`，保留 `text` 供播报与落盘。

**Step 2: 编译 + clippy + 既有测试**

```bash
cargo check -p zapmomo-app && cargo clippy -p zapmomo-app -- -D warnings && cargo test -- --test-threads=1
```

**Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(dsh): 管线接入 TTS 播报（voice 会话互斥、队列 1）与对话记录落盘"
```

---

### Task 13: 设置页 DshSection 区块（TDD）

**Files:**
- Create: `src-tauri/frontend/src/components/settings/DshSection.tsx`
- Test: `src-tauri/frontend/src/components/settings/DshSection.test.tsx`
- Modify: `src-tauri/frontend/src/pages/SettingsPage.tsx`

**Step 1: 写失败测试**（DshSection.test.tsx，mock 模式仿 ShortcutsSection.test / useVoiceSession.test）

```tsx
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DshSection } from "./DshSection";

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn((event: string, cb: (e: { payload: unknown }) => void) =>
    Promise.resolve(() => {}),
  ),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ success: vi.fn(), error: vi.fn() }),
}));

const baseInfo = {
  enabled: true,
  port: 0,
  voice_enabled: true,
  record_to_history: true,
  running: true,
  actual_port: 52341,
  discovery_path: "/tmp/dsh-bridge.json",
};

describe("DshSection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockReset();
  });

  it("载入配置并显示运行端口", async () => {
    invokeMock.mockResolvedValueOnce(baseInfo);
    render(<DshSection />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_dsh_config"));
    expect(screen.getByText(/52341/)).toBeTruthy();
  });

  it("总开关调用 set_dsh_enabled", async () => {
    invokeMock.mockResolvedValue(baseInfo);
    render(<DshSection />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    const toggles = screen.getAllByRole("switch");
    await userEvent.click(toggles[0]);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_dsh_enabled", { enabled: false }),
    );
  });

  it("测试播报按钮调用 test_dsh_announce", async () => {
    invokeMock.mockResolvedValue(baseInfo);
    render(<DshSection />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    await userEvent.click(screen.getByRole("button", { name: /测试播报/ }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("test_dsh_announce"));
  });
});
```

**Step 2: 跑测试确认失败**

Run: `pnpm -C src-tauri/frontend exec vitest run src/components/settings/DshSection.test.tsx`

**Step 3: 实现 DshSection.tsx**

```tsx
import { Bot } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useToast } from "@/components/ui/toast";
import { api, onDshBridgeStatus } from "@/lib/tauri";
import type { DshConfigInfo } from "@/types/tauri";

/**
 * 设置页「外部感知（dsh 桥）」区块。
 *
 * dsh = deepseek-harness：其 Cordis 插件把任务事件 POST 到本应用的
 * loopback 桥（端口见发现文件 ~/.zapmomo/runtime/dsh-bridge.json），
 * 桌宠以气泡+语音播报。本区块提供开关、运行状态与测试播报。
 */
export function DshSection() {
  const toast = useToast();
  const [info, setInfo] = useState<DshConfigInfo | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = useCallback(() => {
    void api
      .getDshConfig()
      .then(setInfo)
      .catch((e) => toast.error(String(e)));
  }, [toast]);

  useEffect(reload, [reload]);

  // 运行状态实时同步（桥启动/停止事件）
  useEffect(() => {
    const unlisten = onDshBridgeStatus((s) => {
      setInfo((prev) => (prev ? { ...prev, running: s.running, actual_port: s.port } : prev));
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  const toggleEnabled = async (enabled: boolean) => {
    setBusy(true);
    try {
      await api.setDshEnabled({ enabled });
      setInfo((prev) => (prev ? { ...prev, enabled } : prev));
      toast.success(enabled ? "dsh 桥已开启" : "dsh 桥已关闭");
      reload();
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };

  const patchParams = async (params: Partial<DshConfigInfo>) => {
    setBusy(true);
    try {
      await api.setDshParams({
        params: {
          voice_enabled: params.voice_enabled,
          record_to_history: params.record_to_history,
        },
      });
      setInfo((prev) => (prev ? { ...prev, ...params } : prev));
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (!info) return null;

  return (
    <section className="space-y-3">
      <h2 className="flex items-center gap-2 text-base font-semibold">
        <Bot className="size-4" />
        外部感知 · dsh 桥
      </h2>
      <p className="text-sm text-text-muted">
        接收 deepseek-harness 插件推送的任务事件，桌宠以气泡+语音播报。
        {info.running && info.actual_port
          ? ` 桥运行中 · 端口 ${info.actual_port}。`
          : " 桥未启动。"}
      </p>
      <div className="space-y-2">
        <label className="flex items-center justify-between gap-4 text-sm">
          <span>启用 dsh 桥</span>
          <Switch
            checked={info.enabled}
            disabled={busy}
            onCheckedChange={(v) => void toggleEnabled(v)}
          />
        </label>
        <label className="flex items-center justify-between gap-4 text-sm">
          <span>事件语音播报（语音会话进行中自动静音）</span>
          <Switch
            checked={info.voice_enabled}
            disabled={busy || !info.enabled}
            onCheckedChange={(v) => void patchParams({ voice_enabled: v })}
          />
        </label>
        <label className="flex items-center justify-between gap-4 text-sm">
          <span>写入对话记录</span>
          <Switch
            checked={info.record_to_history}
            disabled={busy || !info.enabled}
            onCheckedChange={(v) => void patchParams({ record_to_history: v })}
          />
        </label>
      </div>
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          disabled={!info.enabled}
          onClick={() =>
            void api
              .testDshAnnounce()
              .then(() => toast.success("已发送测试播报，看一眼桌宠～"))
              .catch((e) => toast.error(String(e)))
          }
        >
          测试播报
        </Button>
      </div>
    </section>
  );
}
```

**Step 4: SettingsPage.tsx 挂载**

import 区追加 `import { DshSection } from "@/components/settings/DshSection";`
`<ShortcutsSection />` 之后：

```tsx
      {/* 外部感知（dsh 桥） */}
      <DshSection />
```

> 注意：SettingsPage.test.tsx 若因新增区块（invoke mock 未覆盖 get_dsh_config 返回 undefined）
> 失败，在该测试的 invoke mock 里补 `get_dsh_config` 的返回值（形状同 baseInfo），
> 不删既有用例。

**Step 5: 跑测试确认通过** → `pnpm -C src-tauri/frontend test:run`

**Step 6: Commit**

```bash
git add src-tauri/frontend/src/components/settings/DshSection.tsx \
        src-tauri/frontend/src/components/settings/DshSection.test.tsx \
        src-tauri/frontend/src/pages/SettingsPage.tsx src-tauri/frontend/src/pages/SettingsPage.test.tsx
git commit -m "feat(dsh): 设置页「外部感知」区块（开关/运行状态/语音与落盘选项/测试播报）"
```

---

### Task 14: 阶段 2 验收

**Step 1: 全量检查**（同 Task 10 Step 1 全套命令）Expected: 全绿

**Step 2: 手测**

```bash
pnpm tauri dev
# 1. 设置页 →「外部感知 · dsh 桥」→ 点「测试播报」→ 气泡 + 语音（0.5~1s 延迟）+ 对话记录新增一条
# 2. 快速连点两次 → 第二次语音可能被排队/丢弃，气泡正常
# 3. 开启语音会话（唤醒词唤醒进入对话）后再点测试播报 → 只气泡不出声
# 4. 关闭「事件语音播报」开关 → 再点测试播报 → 只气泡
# 5. 关闭「写入对话记录」→ 测试播报 → 对话记录不再新增
# 6. 关闭总开关 → 「桥未启动」，curl 旧端口失败，~/.zapmomo/runtime/dsh-bridge.json 被删
```

**Step 3: Commit（如有修复）**

---

## 阶段 3：dsh 侧插件 + 端到端联调

> 插件源码放 dsh 侧（**不在本仓库**）：`~/.dsh/plugins/zapmomo-bridge/`。
> 每一步先对照 dsh 仓库 `/Users/nemo/github/deepseek-harness` 的
> `docs/cookbook/extension-cookbook.md` 与 `docs/event-producer-consumer.md` 核对
> 事件名/payload/挂载方式——探索结论（`agent/status`、`session/event` 的 `turn/end`
> 带 `reason.kind`）有 file:line 依据，但以现场版本为准。

### Task 15: log-only 插件——核对 cordis 事件 payload

**Files（dsh 侧）:**
- Create: `~/.dsh/plugins/zapmomo-bridge/package.json`
- Create: `~/.dsh/plugins/zapmomo-bridge/src/index.ts`

**Step 1: 最小插件骨架**

package.json：

```json
{
  "name": "zapmomo-bridge",
  "version": "0.1.0",
  "private": true,
  "main": "src/index.ts"
}
```

src/index.ts（log-only 版）：

```ts
import type { Context } from 'cordis'

export const name = 'zapmomo-bridge'

export interface Config {}

// 事件 payload 形状以日志实测为准；先用宽松类型
export function apply(ctx: Context, _config: Config) {
  const logger = ctx.logger('zapmomo-bridge')

  ctx.on('agent/status', (...args: unknown[]) => {
    logger.info('agent/status:', JSON.stringify(args))
  })

  ctx.on('session/event', (ev: { type?: string; [k: string]: unknown }) => {
    if (ev?.type === 'turn/end' || ev?.type === 'user/message') {
      logger.info('session/event:', JSON.stringify(ev))
    }
  })
}
```

**Step 2: 挂载并观察**

```bash
# 挂载方式二选一（以 extension-cookbook 为准）：
dsh plugin --profile web add ~/.dsh/plugins/zapmomo-bridge
# 或在 ~/.dsh/profiles/web/cordis.patch.yml 追加 zapmomo-bridge 条目

dsh web   # 前台跑，观察 stderr 日志
# 另开终端或 Web UI 发起一个任务，记录：
# - agent/status 的字段名（sessionId? running?）
# - turn/end 的 data.reason 结构（kind 字段路径）
# - user/message 的文本字段路径（供 title 用）
```

**Step 3: 按实测修正 Task 16 的字段访问路径**（计划中的假定路径：`payload.sessionId`、
`ev.data.reason.kind`、`ev.data.content`——以日志为准）

### Task 16: 完整插件——POST 事件到 ZapMomo

**Files（dsh 侧）:**
- Modify: `~/.dsh/plugins/zapmomo-bridge/src/index.ts`

**实现**（字段路径按 Task 15 实测修正）：

```ts
import type { Context } from 'cordis'
import { readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

export const name = 'zapmomo-bridge'

export interface Config {}

/** ZapMomo 桥发现文件路径 */
const BRIDGE_FILE = join(homedir(), '.zapmomo/runtime/dsh-bridge.json')

interface BridgeInfo { port: number; token: string }

/** 读发现文件；ZapMomo 未运行时返回 null（静默跳过）。 */
function bridge(): BridgeInfo | null {
  try {
    return JSON.parse(readFileSync(BRIDGE_FILE, 'utf8')) as BridgeInfo
  } catch {
    return null
  }
}

/** fire-and-forget POST：超时 1s、所有异常吞掉——插件绝不影响 dsh 宿主。 */
function post(type: string, sessionId: string, extra: Record<string, unknown> = {}): void {
  const info = bridge()
  if (!info) return
  const url = `http://127.0.0.1:${info.port}/dsh/events`
  fetch(url, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${info.token}`,
      host: `127.0.0.1:${info.port}`,
    },
    body: JSON.stringify({ type, sessionId, time: Date.now(), ...extra }),
    signal: AbortSignal.timeout(1000),
  }).catch(() => {})
}

export function apply(ctx: Context, _config: Config) {
  const logger = ctx.logger('zapmomo-bridge')
  // 会话 → 最近一条用户指令摘要（模板台词的 title）
  const titleBySession = new Map<string, string>()

  ctx.on('session/event', (ev: { type?: string; sessionId?: string; data?: unknown; [k: string]: unknown }) => {
    if (ev.type === 'user/message') {
      // 字段路径按 Task 15 实测调整（假定 data.content 或 data.text）
      const text = String((ev.data as Record<string, unknown>)?.content ?? '')
      if (text.trim() && ev.sessionId) {
        titleBySession.set(ev.sessionId, text.trim().slice(0, 40))
      }
      return
    }
    if (ev.type !== 'turn/end' || !ev.sessionId) return
    const data = (ev.data ?? {}) as Record<string, unknown>
    const reason = String((data.reason as Record<string, unknown>)?.kind ?? '')
    const title = titleBySession.get(ev.sessionId)
    if (reason === 'completed') {
      post('task-finished', ev.sessionId, { title, reason })
    } else if (reason === 'error') {
      post('task-failed', ev.sessionId, {
        title,
        reason,
        detail: String((data.reason as Record<string, unknown>)?.detail ?? '').slice(0, 200),
      })
    } else {
      // aborted / interrupted / max-tokens / blocked → 中断
      post('task-interrupted', ev.sessionId, { title, reason })
    }
  })

  ctx.on('agent/status', (payload: { sessionId?: string; status?: string; running?: boolean }) => {
    // 只报「开始」：结束由 turn/end 携带原因上报，避免同一次结束推两条
    if (payload.running === true && payload.sessionId) {
      post('task-started', payload.sessionId, { title: titleBySession.get(payload.sessionId) })
    } else {
      logger.debug('agent/status ignored:', JSON.stringify(payload))
    }
  })
}
```

**验证:** `dsh web` 跑真任务，ZapMomo 桌宠出现开始/完成气泡（ZapMomo 日志
`~/.zapmomo/logs/app.log` 有 `dsh 事件播报` 行）。

### Task 17: 端到端验收清单

```bash
# 前置：ZapMomo（pnpm tauri dev 或安装版）+ dsh web（插件已挂载）
# 1. dsh Web UI 提交一个短任务（如 "用一句话介绍你自己"）
#    → 桌宠气泡「「…」开工啦…」+ 语音
#    → 完成后「「…」搞定啦！」+ 语音；对话记录页新增两条 assistant 记录
# 2. 提交一个会失败的任务（如让模型调用不存在的工具/断网）
#    → 「task-failed」失败台词
# 3. 任务运行中手动停止（dsh UI 取消）
#    → 「task-interrupted」中断台词
# 4. 退出 ZapMomo → dsh 继续跑任务，dsh 无报错（插件静默跳过）
# 5. 重启 ZapMomo → 桥自启、发现文件重建，dsh 任务事件恢复播报
# 6. dsh web 退出重登 → 会话新任务正常触发（title 正确）
```

全部通过后：

```bash
git add -A && git commit -m "docs(dsh): 阶段3联调记录" --allow-empty
```

---

## 已知边界（记录到 PR 描述）

- 进程被 kill -9 时发现文件可能残留 → 下次启动清陈旧（已实现）
- TTS 模型未就绪 → 只出气泡不语音，warn 日志（每次事件重试构建播报器）
- voice 会话运行中（含待唤醒）→ 事件只出气泡不出声
- 播报队列容量 1：正在播 + 已排队 1 条时新事件只留气泡
- 插件 title 取会话最近一条 user/message 前 40 字符
- 未来扩展缝：`src/dsh/` 事件解析/台词/节流为纯函数，接入 Claude Code
  （hooks→curl）/ Codex（notify→curl）/ 飞书（connector 连出）时复用管线与展示层
