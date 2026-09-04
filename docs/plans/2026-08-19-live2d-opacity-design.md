# Live2D 模型透明度调节 · 设计文档

- 日期：2026-08-19
- 状态：已确认（与用户逐节评审通过）
- 范围：角色窗口右键菜单、托盘菜单、设置页三处入口调节模型透明度，持久化并即时生效

## 1. 需求

- 用户可以在三处调节 Live2D 模型透明度：
  1. **角色窗口右键菜单**（原生菜单，5 档：100% / 80% / 60% / 40% / 20%）
  2. **托盘菜单**（同样 5 档子菜单；顺带补齐「窗口尺寸」子菜单，与右键菜单对称）
  3. **设置页**（Slider 连续调节，20%~100%，step 5）
- 透明度持久化到 `~/.zapmomo/settings.toml`，重启后保持。
- 菜单档位显示勾选态（`CheckboxMenuItem`），「窗口尺寸」子菜单顺带升级为勾选态。
- 明确不做（YAGNI）：
  - 不加 alt+滚轮快捷调节（scale 有 cmd/ctrl+滚轮，透明度保持三处入口）。
  - 不做窗口级 alpha（Tauri 2 无跨平台 API），透明度只作用于前端渲染层。

## 2. 现状：scale（窗口尺寸）链路

透明度完全复刻 scale 的四层对称结构：

| 层 | 文件 | scale 的做法 |
|---|---|---|
| 配置持久化 | `src/config/settings.rs:372` | `Live2dSettings.window_scale: Option<f64>` |
| Rust 后端 | `src-tauri/src/lib.rs:2136` | `apply_companion_scale()`：写设置 + `emit("companion-scale-changed")` |
| 菜单入口 | `src-tauri/src/lib.rs:2218` | 右键菜单「窗口尺寸」子菜单 6 档；`handle_menu()` 统一分发 |
| 前端应用 | `CompanionRoot.tsx` / `CompanionPage.tsx` | 角色窗口监听事件 + resize；设置页 Slider |

关键差异：scale 调整窗口物理尺寸（Tauri `setSize`）；透明度无对应窗口 API，改用**前端 CSS opacity**。后端职责不变：只存数字 + 发事件。

已知现状：托盘菜单（`lib.rs:3482`）目前无尺寸入口，本次补齐。

## 3. 架构与数据流

```
设置页 Slider ──┐
右键菜单 5 档 ──┼→ set_companion_opacity / handle_menu
托盘菜单 5 档 ──┘        │
                         ▼
              写 settings.toml [live2d].window_opacity
                         │ emit("companion-opacity-changed", 0.8)
                         ▼
              CompanionRoot 监听 → setState → wrapper div style.opacity
```

交互特性（非缺陷，实现前已知）：调低透明度仅是视觉变化，模型点击/拖拽区域不变。

**透明度只作用于模型，`VoiceStatusDot`（语音状态点）保持不透明**——状态提示跟随变透明会失去意义。因此 opacity 应用在包裹 `Live2dStage` 的 wrapper div 上，而非根容器。

## 4. 实施细节

### 4.1 配置层（`src/config/settings.rs`）

`Live2dSettings` 加字段（serde 属性与 `window_scale` 一致）：

```rust
/// 角色窗口透明度（1.0 = 不透明；缺省视为 1.0）
#[serde(default, skip_serializing_if = "Option::is_none")]
pub window_opacity: Option<f64>,
```

### 4.2 Rust 后端（`src-tauri/src/lib.rs`）

1. `apply_companion_opacity()`（仿 `apply_companion_scale`）：load → clamp 0.2~1.0 → 写 `window_opacity` → save → `emit("companion-opacity-changed", opacity)`。
2. `set_companion_opacity` command：单行转发，注册到 `invoke_handler`。
3. `opacity_from_id()`（仿 `scale_from_id`）：`opacity_100/80/60/40/20` → `1.0/0.8/0.6/0.4/0.2`；`handle_menu` fallback 分支加 `else if let Some(opacity) = opacity_from_id(id)`。
4. 菜单构建：
   - `build_companion_menu`（右键菜单）：读 settings 取当前 scale/opacity，「窗口尺寸」+「透明度」两个子菜单均用 `CheckboxMenuItem`，当前档位 checked。
   - 托盘菜单：加「窗口尺寸」「透明度」两个子菜单（同样勾选态），仍走 `handle_menu` 分发。
5. `get_live2d_config` 返回结构加 `window_opacity` 字段。

### 4.3 前端类型与绑定

```ts
// types/tauri.ts: Live2dConfig 加
window_opacity: number | null;

// lib/tauri.ts
setCompanionOpacity: (args: { opacity: number }) => invoke<void>("set_companion_opacity", args),

export function onCompanionOpacityChanged(handler: (opacity: number) => void): Promise<UnlistenFn> {
  return listen<number>("companion-opacity-changed", (e) => handler(e.payload));
}
```

### 4.4 角色窗口（`CompanionRoot.tsx`）

```tsx
const [opacity, setOpacity] = useState(1.0);
// 初始化：config.window_opacity ?? 1.0（与 scale 同一处 useEffect）
// 监听：onCompanionOpacityChanged((v) => setOpacity(v))
// 应用：
<div style={{ opacity }}>          {/* 新 wrapper，只包模型 */}
  <Live2dStage ... />
</div>
```

与 scale 的差异：角色窗口内无透明度调节入口（不做滚轮快捷键），**纯被动接收事件**，不需要 `applyOpacity`。

### 4.5 设置页（`CompanionPage.tsx`）

- `percent` 旁加 `opacityPercent` state（初始 100，从 `getLive2dConfig` 读 `window_opacity`）。
- 「桌宠尺寸」旁加「透明度」Slider：`min=20 max=100 step=5`；`handleOpacityChange` → clamp → `api.setCompanionOpacity({ opacity: v / 100 })`。
- 与 scale 现状一致：菜单改档位后设置页不实时同步，下次打开重新读 config。

## 5. 测试与验收

### Rust 测试

- `settings.rs`：serde roundtrip 用例加 `window_opacity: Some(0.6)`；None 时 TOML 不含该字段。
- `lib.rs`：`opacity_from_id` 映射单测；`apply_companion_opacity` clamp 边界（< 0.2、> 1.0）单测。

### 前端测试（仿 `CompanionPage.test.tsx:130` scale 用例）

- 「选中伙伴后显示透明度控制，拖动滑块调用 set_companion_opacity」。
- mock config 返回 `window_opacity: 0.8` → 初始显示 80%。

### 验收命令

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
cargo clippy -p zapmomo-app -- -D warnings
# 前端 tsc / vitest（在 src-tauri/frontend 下）
```

手动验收：三处入口改透明度 → 模型即时变化；重启后保持；`window_scale` 现有行为（右键/滚轮/设置页）不回归。
