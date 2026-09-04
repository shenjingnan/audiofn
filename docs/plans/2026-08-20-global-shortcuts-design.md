# 全局快捷键自定义功能设计

- 日期：2026-08-20
- 状态：设计已评审通过，待实施
- 目标分支：`feature/global-shortcuts`（待创建）

## 1. 背景与目标

ZapMomo 是桌面 AI 伴侣（桌宠）。桌宠窗口是非激活面板（macOS 上转 NSPanel），拿不到键盘焦点，用户无法通过应用内键位操作它；当前应用也**没有任何快捷键基建**：

- 未安装 `tauri-plugin-global-shortcut`（`src-tauri/Cargo.toml` 仅有 `tauri-plugin-dialog`）
- 配置文件 `~/.zapmomo/settings.toml` 没有 `[shortcuts]` 分节
- 现有键位处理零散且均为窗口内局部处理：macOS 菜单 `Cmd+,` 开设置（`lib.rs:3964`）、各测试对话框 Esc 关闭、`LlmTestDialog` Enter 发送等

**目标**：在设置页新增「快捷键」区块，允许用户为高频操作自定义**系统级全局快捷键**——任意应用聚焦时均可触发。

## 2. 关键决策

| 决策点 | 结论 | 理由 |
| --- | --- | --- |
| 作用域 | 全局为主 | 桌宠窗口拿不到键盘焦点，应用内快捷键对核心场景无效；设置页内输入框、对话框等保留现有应用内键位 |
| 首批操作 | 第一梯队 4 个 | 覆盖最高频场景，控制首版范围 |
| 默认策略 | 默认不注册任何快捷键 | 配置为空时零注册，老用户升级零冲突、零惊喜；录入后才生效 |

### 首批支持的操作

| 操作 | action 标识 | 实现现状 |
| --- | --- | --- |
| 显示/隐藏桌宠 | `toggle_companion` | 内部函数 `toggle_companion_window()` 已有（托盘在用），分发时直接调用 |
| 语音会话 开/关 | `toggle_voice_session` | `start_voice_session` / `stop_voice_session` / `is_voice_session_running` 均已有，分发时查询状态二选一 |
| 手动打断播报 | `interrupt_reply` | 需薄封装：`stop_tts` + `stop_llm`；打断后 voice 会话回到 Armed 待唤醒态而非整场停掉（若 voice 状态机已有「结束当前轮」内部路径则复用） |
| 打开设置 | `open_settings` | `show_settings_window()` 已有 |

**不绑快捷键**：重启/退出（全局键误触代价高）、导入伙伴/下载模型等（低频且需 UI 反馈）。置顶切换、尺寸/透明度档位调节列为后续候选。

## 3. 架构与数据流

```
┌─ 设置页「快捷键」区块 ──────────────────────┐
│  [显示/隐藏桌宠]  [点击录入…]  [清除]        │
│  [语音会话 开/关]  [点击录入…]  [清除]       │
│  [打断播报]       [点击录入…]  [清除]        │
│  [打开设置]       [点击录入…]  [清除]        │
└──────────────┬─────────────────────────────┘
               │ set_shortcut(action, "CmdOrCtrl+Shift+Z")
               ▼
┌─ Rust 侧 ───────────────────────────────────┐
│ 1. 校验 accelerator 格式 + 应用内查重        │
│ 2. 先注册（tauri-plugin-global-shortcut）    │
│    ├─ 成功 → 旧快捷键解绑 → 写入 [shortcuts] │
│    └─ 失败 → 返回错误，配置不动              │
│ 3. 快捷键触发 → dispatch_shortcut(action)   │
│    ├─ toggle_companion     → 现有内部函数     │
│    ├─ toggle_voice_session → start/stop 二选一│
│    ├─ interrupt_reply      → stop_tts+stop_llm│
│    └─ open_settings        → show_settings_window│
└──────────────────────────────────────────────┘
```

核心原则：**先注册成功、再落盘配置**。键位被系统或其他应用占用时（注册返回 Err），设置页弹错误提示，配置保持原值——杜绝「界面显示已绑定但实际不生效」的假状态。

## 4. 配置模型

沿用现有 `AppConfig` 可选分节模式（`src/config/settings.rs`）：

```toml
[shortcuts]
toggle_companion = "CmdOrCtrl+Shift+Z"
toggle_voice_session = "CmdOrCtrl+Shift+V"
interrupt_reply = "CmdOrCtrl+Shift+X"
open_settings = "CmdOrCtrl+Shift+,"
```

- `ShortcutsSettings` 四个字段全部 `Option<String>` + `skip_serializing_if`，缺分节/空值 = 该操作无快捷键
- accelerator 存插件标准字符串（`Modifier+Key`，如 `CmdOrCtrl+Shift+Z`）
- 全部为空时启动阶段不注册任何全局快捷键，老用户升级后行为零变化

## 5. 设置页交互

**入口**：设置页现有 Section（通用 / 模型下载 / 存储位置）之后新增「快捷键」Section，沿用现有 Section 组件风格，不新增路由页面。

**录制流程**：

1. 行内按钮显示当前绑定的 accelerator（如 `⌘⇧Z`；未绑定时显示灰色「未设置」）
2. 点击进入**录制态**：按钮变为「按下组合键…」，监听 `keydown`
3. 只接受**带修饰键**的组合（Ctrl/Cmd/Alt/Option + 字母/数字）；裸按键忽略；按 Esc 取消录制
4. 捕获后立即调 `set_shortcut`：
   - 成功 → 按钮刷新为新键位，绿色对勾短暂反馈
   - 失败 → 红色错误文案（区分「已绑定到其他操作」与「注册失败，可能被其他应用占用」），保持原值
5. 每行右侧「清除」按钮 → `clear_shortcut(action)` → 解绑 + 配置字段置空

**两道冲突防线**：

- 前端：录制时本地比对其他操作的键位，重复直接拦截（不发请求）
- 后端：`set_shortcut` 内再查 `[shortcuts]` 全部字段，重复返回 `AlreadyBound` 错误兜底

## 6. Rust 侧 command 与启动注册

**新增 3 个 command**（沿用 `get_hide_dock_icon` / `set_hide_dock_icon` 的读写惯例）：

| command | 行为 |
| --- | --- |
| `get_shortcuts` | 读 `[shortcuts]`，返回 action → accelerator 映射 |
| `set_shortcut(action, accelerator)` | 校验格式与查重 → 先注册成功 → 解绑旧键 → 落盘 |
| `clear_shortcut(action)` | 解绑 → 字段置 None → 落盘 |

- `action` 为枚举 `ShortcutAction { ToggleCompanion, ToggleVoiceSession, InterruptReply, OpenSettings }`，序列化为 snake_case 字符串
- 错误类型区分 `InvalidAccelerator` / `AlreadyBound(action)` / `RegisterFailed(reason)`，前端据此显示不同文案

**启动注册**：`setup` 阶段读 `[shortcuts]` 逐个 `register`；单个失败仅 `warn!` 日志（附 action 与原因），不阻塞启动，其余照常注册。

**依赖与权限**：`src-tauri/Cargo.toml` 加 `tauri-plugin-global-shortcut = "2"`；`capabilities/` 补权限声明。

## 7. 错误处理边界

| 场景 | 行为 |
| --- | --- |
| 录入时键位被其他应用占用 | 注册 Err → 前端提示，配置不落盘 |
| 启动时某键注册失败（键位后被其他软件占用） | `warn!` 日志，跳过该项，其余正常，应用照常启动 |
| 配置文件手改出现非法 accelerator | 启动注册同样走 Err 路径 → 跳过 + 日志；设置页显示「未生效」 |
| 全裸单键（如只按 `Z`） | 前端录制态忽略；后端校验拒绝（双保险） |
| 应用退出 | 全局快捷键随进程自动释放，无需手动清理（插件保证） |

## 8. 测试方案

- **Rust 单测**（CI 可跑的纯逻辑，不测真实系统注册）：
  - `ShortcutsSettings` TOML 序列化/反序列化（含空值、缺分节）
  - `ShortcutAction` 枚举 ↔ snake_case 字符串互转
  - accelerator 合法性校验（裸键拒绝、格式错误拒绝）
  - 同分节查重逻辑
- **前端 Vitest**（录制组件）：
  - 按键捕获与组合键展示
  - Esc 取消、裸按键忽略
  - 本地冲突拦截提示
  - `set_shortcut` 失败时保持原值并显示错误
  - 清除按钮调用
  - mock `@tauri-apps/api`（Vitest 4 注意：构造器 mock 用 `function` 实现，不用箭头函数）
- **手动验证清单**（macOS 真机 `pnpm tauri dev`）：
  - 录入四个快捷键后，聚焦其他应用逐一触发
  - 重启应用后配置仍生效
  - 与其他应用（如 Raycast）抢同一键位时的错误提示

## 9. 实施阶段（概要）

1. **Rust 配置层**：`ShortcutsSettings` + 序列化测试
2. **Rust command 层**：3 个 command + `dispatch_shortcut` + `interrupt_reply` 封装 + 单测
3. **插件接入**：依赖、权限、setup 启动注册
4. **前端**：设置页「快捷键」Section + 录制组件 + Vitest
5. **真机验证**：手动清单过一遍
