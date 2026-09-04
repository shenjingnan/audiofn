# 桌宠点击穿透（方案 A：开关式全窗口穿透）设计

日期：2026-08-20
状态：已评审（方案选型见下），实施中

## 背景与问题

桌宠（Live2D 模型）运行在独立的 `companion` 窗口：`transparent + always_on_top +
skip_taskbar + 无边框`（`src-tauri/src/lib.rs` setup）。操作系统的鼠标命中测试作用于
**整个窗口矩形**，不感知像素透明度——模型周围大片视觉透明区域同样拦截点击，挡住
身后其他应用 / 设置窗口的元素。

DOM 层 `pointer-events: none` 无法解决：它只影响事件在 webview 内部的分发，事件本身
已被本窗口消费，不会跨窗口转发。因此必须在 **OS 窗口层** 实现。

## 方案选型

| 方案 | 形态 | 结论 |
| --- | --- | --- |
| A. 开关式全窗口穿透 | 用户手动开关，开启后窗口对所有鼠标事件透明 | **本期实施**：成本低、风险小 |
| B. 智能像素级穿透 | 光标在不透明像素上可交互、透明区域自动穿透 | Phase 2 另立方案（需全局光标轮询 + alpha 判定，复杂度高一个量级） |
| C. 裁紧窗口包围盒 | 复用 `computeModelBounds` 缩小窗口矩形 | 长期辅助优化，不在本期 |

A 的核心依赖：Tauri 2 `WebviewWindow::set_ignore_cursor_events(bool)`

- Windows：`WS_EX_TRANSPARENT`（companion 已是 layered 透明窗口，直接生效）
- macOS：`NSWindow.ignoresMouseEvents`（与 tauri-nspanel 转出的非激活 panel 为同一底层
  对象，理论兼容，**需真机验证**）
- Linux：GTK input shape（X11 稳定；Wayland 需真机验证）

## 行为定义

- 开启：`companion` 窗口对所有鼠标事件透明——点击直达身后任意窗口的内容；拖动 /
  cmd(ctl)+滚轮缩放 / 右键原生菜单随之失效（模型成为纯视觉挂件，渲染与动画不受影响）。
- 关闭：恢复现有全部交互。
- 持久化到 `~/.zapmomo/settings.toml` 的 `[live2d].click_through`，重启后恢复。
- **关闭入口必须在窗口外**（开启后窗口收不到鼠标）：设置页「伙伴」面板 + 托盘菜单。
- 语音状态点随窗口一并穿透（可接受，纯状态展示）。

## 实施设计

### 1. 配置（`src/config/settings.rs`）

`Live2dSettings` 增加可缺省字段（与 `window_opacity` 同款模式）：

```rust
/// 角色窗口点击穿透（true = 鼠标事件全部穿透到身后窗口；缺省视为 false）
#[serde(default, skip_serializing_if = "Option::is_none")]
pub click_through: Option<bool>,
```

单测：扩展 `test_live2d_settings_serde_roundtrip`（Some(true) 往返 / None 不写盘）。

### 2. Rust command 与启动恢复（`src-tauri/src/lib.rs`）

- `apply_companion_click_through(app, enabled)`：写 settings → 对 companion 窗口
  `set_ignore_cursor_events(enabled)` → `rebuild_tray_menu(app)` 刷新托盘勾选态。
  不发事件（companion 前端零改动，穿透不影响渲染）。仿 `apply_companion_opacity`。
- `#[tauri::command] set_companion_click_through(app, enabled)`，注册进 invoke_handler。
- setup 中 `companion` 建窗（含 NSPanel 转换）后：持久化值为 true 时立即应用。
  窗口 hide/show 不销毁窗口对象，穿透状态自然保留，无需在 `toggle_companion_window`
  里处理。
- `Live2dConfigInfo` 增加 `click_through: Option<bool>`，`get_live2d_config` 返回。

### 3. 菜单入口（`src-tauri/src/lib.rs`）

- 新增 `build_click_through_item`（`CheckMenuItem`，id `click_through`，构建时读
  settings 打勾），加入：
  - companion 右键菜单 `build_companion_menu`（每次右键重建，勾选态天然正确）
  - 托盘菜单 `build_tray_menu`（apply 时 `rebuild_tray_menu` 刷新）
- `handle_menu` 增加 `"click_through"` 分支：取反当前值并应用。

### 4. 前端（settings 窗口）

- `types/tauri.ts`：`Live2dConfigInfo` 加 `click_through: boolean | null`。
- `lib/tauri.ts`：`api.setCompanionClickThrough({ enabled })`。
- `pages/CompanionPage.tsx`：预览卡片头部（尺寸/透明度滑杆下方）加「点击穿透」
  Switch 行 + 说明文案（明确开启后拖动/缩放/右键菜单失效）。开关为窗口级行为，
  未选中伙伴时也可见。初始值来自 `getLive2dConfig`。

### 5. 明确不做（YAGNI）

- 像素级智能穿透（Phase 2）
- 默认全局快捷键绑定（#122 自定义快捷键框架已支持用户自绑，有需求再加）
- `companion-click-through-changed` 事件广播（无消费者）

## 验收标准

1. 设置页开关开启后：点击模型及其透明边缘区域，事件落到身后窗口；重启应用保持开启。
2. 开启状态下拖动 / cmd(ctl)+滚轮 / 右键菜单均无响应；托盘菜单与设置页可关闭并恢复交互。
3. 托盘菜单与右键菜单的「点击穿透」勾选态与实际状态一致（任一入口切换后另一处同步）。
4. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` 通过；
   前端 `vitest run` + `tsc -b` 通过。
5. 人工验证（CI 无法覆盖 OS 鼠标行为）：macOS 重点验证 NSPanel 兼容；Windows / Linux
   各验证一次开启/关闭循环。

## 风险

| 风险 | 缓解 |
| --- | --- |
| macOS NSPanel 与 `ignoresMouseEvents` 兼容性 | 理论同对象；真机验证列入验收 |
| Linux Wayland input shape 行为 | 真机验证；异常则该平台文档标注限制 |
| 用户开启后找不到关闭入口 | 设置页 + 托盘双入口；开关文案明示 |
