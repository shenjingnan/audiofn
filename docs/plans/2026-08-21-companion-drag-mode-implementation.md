# 桌宠拖拽模式（直接/修饰键拖动）Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 桌宠 Live2D 窗口移动支持两种模式——`direct`（按住左键直接拖动，现状默认）与 `modifier`（需按住 cmd/Ctrl 才能拖动），设置页提供开关，旧配置零迁移。

**Architecture:** 完整照抄位置锁定（`locked`，PR #131）的链路：`settings.toml` 的 `[live2d].drag_mode` 字段 → Tauri command `set_companion_drag_mode` 写入并 emit `companion-drag-mode-changed` 事件 → 前端 `CompanionRoot` 订阅后在 mousedown 守卫里判断修饰键。锁定（`locked`）优先于拖拽模式；右键/托盘菜单不加入口（仅设置页）。

**Tech Stack:** Rust（serde/TOML、Tauri 2 command + event）、React 19 + TypeScript、Vitest + Testing Library。

**设计文档:** `docs/plans/2026-08-21-companion-drag-mode-design.md`

**与设计文档的一处偏离（已评估）:** 设计文档 §6 提到照抄 `resolve_locked` 写 `resolve_drag_mode`；实施时发现它没有调用者（拖拽模式不进菜单，不需要 `current_*` 读函数），`cargo clippy -- -D warnings` 会报 dead_code。后端单测改由 `settings.rs` 的 serde roundtrip + 旧配置回退测试覆盖（数据层真正的风险点），`lib.rs` 层的 apply 逻辑与 `apply_companion_locked` 完全同构。

**实施补充记录:**
- Task 2 顺带在 `settings.rs` 补了 `test_live2d_drag_mode_invalid_value_rejected` 负向测试（`drag_mode = "bogus"` 时 load_settings 响亮失败）——来自 Task 1 质量审查的 Minor 建议，与 `CompanionWindowLayer` 的既有对称用例，锁定「非法值 fail loud」契约。
- Task 4 质量审查后把 config 恢复写法从 `if (config.drag_mode) setDragMode(...)` 统一为 `setDragMode(config.drag_mode ?? "direct")`（commit bfcf7602）：与 `locked` 的显式兜底对称，消除未来 config refetch 场景下两字段行为分歧。计划 Task 4/5 Step 代码块保留原始写法未回写。
- 最终审查发现 main 上 #132 已把设置区开关说明收敛为 Info icon + Tooltip 规范：合并 main 后把新开关适配为同款（`w-16 shrink-0` label + Tooltip 说明），并将两个测试文件 mock 的 `dragMode` 类型收紧为 `CompanionDragMode | null`。

**工作目录:** `/Users/nemo/Projects/shenjingnan/zapmomo/.claude/worktrees/effervescent-sniffing-karp`（git worktree，分支 `feature/companion-drag-mode`）。前端命令都在 `src-tauri/frontend/` 下执行。

---

### Task 1: Rust 配置层 —— `CompanionDragMode` 枚举与 `drag_mode` 字段

**Files:**
- Modify: `src/config/settings.rs:515`（`CompanionWindowLayer` 枚举后插入新枚举）
- Modify: `src/config/settings.rs:542`（`Live2dSettings.locked` 字段后加字段）
- Test: `src/config/settings.rs:1062-1112`（`test_live2d_settings_serde_roundtrip` 与 `test_load_settings_with_live2d_table`）

**Step 1: 先写失败测试**

`test_live2d_settings_serde_roundtrip`（settings.rs:1062）中：

1. 第一个结构体字面量（1071 行 `locked: Some(true),` 之后）加：

```rust
            drag_mode: Some(CompanionDragMode::Modifier),
```

2. `toml_str` 断言区（1076 行 `assert!(toml_str.contains("locked = true"));` 之后）加：

```rust
        assert!(toml_str.contains("drag_mode = \"modifier\""));
```

3. 反序列化断言区（1079 行后）加：

```rust
        assert_eq!(deserialized.drag_mode, Some(CompanionDragMode::Modifier));
```

4. `none_pos` 结构体字面量（1088 行 `locked: None,` 之后）加：

```rust
            drag_mode: None,
```

5. skip 断言区（1096 行后）加：

```rust
        assert!(!none_toml.contains("drag_mode"));
```

6. 末尾缺省断言（1098 行后）加：

```rust
        assert_eq!(CompanionDragMode::default(), CompanionDragMode::Direct);
```

`test_load_settings_with_live2d_table`（settings.rs:1101）中，1110 行后加：

```rust
            assert_eq!(live2d.drag_mode, None);
```

并把 1108 行注释改为「旧版配置无 click_through / locked / drag_mode 字段 → 反序列化回退 None（视为关闭/直拖）。」

**Step 2: 跑测试确认失败（编译错误即失败）**

```bash
cargo test --lib test_live2d_settings_serde_roundtrip -- --test-threads=1
```

Expected: FAIL——`no field 'drag_mode' on type Live2dSettings` / `cannot find type CompanionDragMode`。

**Step 3: 最小实现**

枚举（settings.rs:515 `CompanionWindowLayer` 的 `}` 之后、517 行 `/// Live2D 角色配置。` 之前插入）：

```rust
/// 角色窗口拖拽模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompanionDragMode {
    /// 直接拖动：按住左键即可移动窗口（默认，现状）。
    #[default]
    Direct,
    /// 修饰键拖动：需按住 cmd（macOS）/ Ctrl（Windows、Linux）才能拖动。
    Modifier,
}
```

字段（settings.rs:542 `pub locked: Option<bool>,` 之后）：

```rust
    /// 角色窗口拖拽模式（direct = 左键直接拖动；modifier = 需按住 cmd/Ctrl；缺省视为 direct）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drag_mode: Option<CompanionDragMode>,
```

**Step 4: 跑测试确认通过**

```bash
cargo test --lib test_live2d_settings_serde_roundtrip -- --test-threads=1
cargo test --lib test_load_settings_with_live2d_table -- --test-threads=1
```

Expected: 两条 PASS。

**Step 5: Commit**

```bash
git add src/config/settings.rs
git commit -m "feat(config): [live2d] 新增 drag_mode 拖拽模式字段（direct/modifier）"
```

---

### Task 2: Rust 后端 —— command、事件与配置透传

**Files:**
- Modify: `src-tauri/src/lib.rs:25-26`（import 加 `CompanionDragMode`）
- Modify: `src-tauri/src/lib.rs:1988`（`Live2dConfigInfo` 加字段）
- Modify: `src-tauri/src/lib.rs:2013`（`get_live2d_config` 透传）
- Modify: `src-tauri/src/lib.rs:2369`（`apply_companion_locked` 后加 `apply_companion_drag_mode`）
- Modify: `src-tauri/src/lib.rs:2542`（`set_companion_locked` 后加 command）
- Modify: `src-tauri/src/lib.rs:4238`（invoke_handler 注册）

**Step 1: 实现（本任务为纯样板同构，无独立逻辑单测，由 Step 2 的编译/clippy/既有测试验证）**

1. import（lib.rs:25）：

```rust
use zapmomo::config::settings::{
    self, AsrSettings, CompanionDragMode, CompanionWindowLayer, CompanionWindowPosition,
    KwsSettings, Live2dSettings,
```

（保持原有行宽风格，`cargo fmt` 会归位。）

2. `Live2dConfigInfo`（1988 行 `locked: Option<bool>,` 后）：

```rust
    drag_mode: Option<CompanionDragMode>,
```

3. `get_live2d_config`（2013 行 `let locked = ...` 后）：

```rust
    let drag_mode = live2d_settings.as_ref().and_then(|l| l.drag_mode);
```

结构体字面量（2024 行 `locked,` 后）加 `drag_mode,`。

4. `apply_companion_drag_mode`（2369 行 `apply_companion_locked` 的 `}` 后）：

```rust
/// 保存并应用角色窗口拖拽模式（内部实现，供 command 调用）。
///
/// modifier 模式仅收紧前端拖动条件（CompanionRoot 的 mousedown → startDragging
/// 需按住 cmd/Ctrl），滚轮缩放与右键菜单不受影响；与 locked 正交（locked 优先，
/// 完全禁止拖动）。拖拽模式不进右键/托盘菜单，无需 rebuild_tray_menu。
fn apply_companion_drag_mode(app: &AppHandle, mode: CompanionDragMode) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let live2d = settings.live2d.get_or_insert_with(Live2dSettings::default);
    live2d.drag_mode = Some(mode);
    settings::save_settings(&settings)?;
    let _ = app.emit("companion-drag-mode-changed", mode);
    Ok(())
}
```

5. command（2542 行 `set_companion_locked` 的 `}` 后）：

```rust
/// 设置并持久化角色窗口拖拽模式（modifier = 需按住 cmd/Ctrl 才能拖动）。
///
/// 由设置面板调用：写入 `~/.zapmomo/settings.toml` 的 `[live2d].drag_mode`，
/// 并通过 `companion-drag-mode-changed` 事件通知角色窗口实时生效。
#[tauri::command]
fn set_companion_drag_mode(app: AppHandle, mode: CompanionDragMode) -> Result<(), String> {
    apply_companion_drag_mode(&app, mode)
}
```

6. invoke_handler（4238 行 `set_companion_locked,` 后）加一行：

```rust
            set_companion_drag_mode,
```

**Step 2: 验证**

```bash
cargo fmt
cargo clippy -p zapmomo-app -- -D warnings
cargo test --lib test_live2d_settings -- --test-threads=1
```

Expected: clippy 无告警；settings 测试 PASS。

**Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(app): 新增 set_companion_drag_mode 命令与 companion-drag-mode-changed 事件"
```

---

### Task 3: 前端类型与 API 封装

**Files:**
- Modify: `src-tauri/frontend/src/types/tauri.ts:113`（`CompanionWindowLayer` 后加类型）
- Modify: `src-tauri/frontend/src/types/tauri.ts:126`（`Live2dConfigInfo.locked` 后加字段）
- Modify: `src-tauri/frontend/src/lib/tauri.ts:191`（api 加方法）
- Modify: `src-tauri/frontend/src/lib/tauri.ts:287`（事件订阅）

**Step 1: 实现**

1. types/tauri.ts（113 行后）：

```typescript
/** 角色窗口拖拽模式：direct（左键直接拖动，默认）/ modifier（需按住 cmd/Ctrl） */
export type CompanionDragMode = "direct" | "modifier";
```

2. `Live2dConfigInfo`（126 行 `locked: boolean | null;` 后）：

```typescript
  /** 拖拽模式（null = 旧后端未返回，视为 direct） */
  drag_mode: CompanionDragMode | null;
```

3. lib/tauri.ts 的 api 对象（191 行 `setCompanionLocked` 后）：

```typescript
  setCompanionDragMode: (args: { mode: CompanionDragMode }) =>
    invoke<void>("set_companion_drag_mode", args),
```

4. 事件订阅（287 行 `onCompanionLockedChanged` 后）：

```typescript
export function onCompanionDragModeChanged(
  handler: (mode: CompanionDragMode) => void,
): Promise<UnlistenFn> {
  return listen<CompanionDragMode>("companion-drag-mode-changed", (e) => handler(e.payload));
}
```

5. 更新 lib/tauri.ts 顶部从 `@/types/tauri` 的类型 import，加入 `CompanionDragMode`。

**Step 2: 验证（类型编译 + 存量测试不受影响）**

```bash
cd src-tauri/frontend && pnpm exec tsc -b && pnpm test:run src/components/CompanionRoot.test.tsx
```

Expected: tsc 零错误；存量测试 PASS（mock 的 config 缺 `drag_mode` 键在运行时无害——TS 接口仅在类型层）。

**Step 3: Commit**

```bash
git add src-tauri/frontend/src/types/tauri.ts src-tauri/frontend/src/lib/tauri.ts
git commit -m "feat(ui): 前端接入 setCompanionDragMode API 与 companion-drag-mode-changed 订阅"
```

---

### Task 4: CompanionRoot 拖拽守卫（TDD）

**Files:**
- Modify: `src-tauri/frontend/src/components/CompanionRoot.tsx`
- Test: `src-tauri/frontend/src/components/CompanionRoot.test.tsx`

**Step 1: 写失败测试**

1. hoisted mock 的 `configState`（test 文件 12 行）改为：

```typescript
    /** get_live2d_config 的 locked / drag_mode 覆盖值（null = 后端未返回该字段）。 */
    configState: { locked: null as boolean | null, dragMode: null as string | null },
```

2. `beforeEach`（70 行 `configState.locked = null;` 后）加 `configState.dragMode = null;`；mock 的 `get_live2d_config` 返回值（85 行 `locked: configState.locked,` 后）加：

```typescript
          drag_mode: configState.dragMode,
```

3. 文件末尾新增 describe：

```tsx
describe("CompanionRoot（拖拽模式）", () => {
  it("缺省（null）视为 direct：裸左键按下触发窗口拖动", async () => {
    configState.dragMode = null;
    render(<CompanionRoot />);
    const container = screen.getByRole("application");
    await waitForConfigApplied();

    fireEvent.mouseDown(container);
    expect(startDraggingMock).toHaveBeenCalledTimes(1);
  });

  it("modifier 模式裸左键按下不触发拖动，按住 cmd 触发", async () => {
    configState.dragMode = "modifier";
    render(<CompanionRoot />);
    const container = screen.getByRole("application");
    await waitForConfigApplied();

    fireEvent.mouseDown(container);
    expect(startDraggingMock).not.toHaveBeenCalled();

    fireEvent.mouseDown(container, { metaKey: true });
    expect(startDraggingMock).toHaveBeenCalledTimes(1);
  });

  it("modifier 模式下 ctrl（Windows/Linux）同样触发拖动", async () => {
    configState.dragMode = "modifier";
    render(<CompanionRoot />);
    const container = screen.getByRole("application");
    await waitForConfigApplied();

    fireEvent.mouseDown(container, { ctrlKey: true });
    expect(startDraggingMock).toHaveBeenCalledTimes(1);
  });

  it("锁定优先于拖拽模式：modifier + 修饰键 + locked 仍不触发", async () => {
    configState.dragMode = "modifier";
    configState.locked = true;
    render(<CompanionRoot />);
    const container = screen.getByRole("application");
    await waitForConfigApplied();

    fireEvent.mouseDown(container, { metaKey: true });
    expect(startDraggingMock).not.toHaveBeenCalled();
  });

  it("companion-drag-mode-changed 事件实时切换拖拽模式", async () => {
    configState.dragMode = "direct";
    render(<CompanionRoot />);
    const container = screen.getByRole("application");
    await waitForConfigApplied();

    fireEvent.mouseDown(container);
    expect(startDraggingMock).toHaveBeenCalledTimes(1);

    // 后端事件：切到 modifier → 裸按被拦截。
    act(() => listenHandlers["companion-drag-mode-changed"]("modifier"));
    fireEvent.mouseDown(container);
    expect(startDraggingMock).toHaveBeenCalledTimes(1);

    // 切回 direct → 拖动恢复。
    act(() => listenHandlers["companion-drag-mode-changed"]("direct"));
    fireEvent.mouseDown(container);
    expect(startDraggingMock).toHaveBeenCalledTimes(2);
  });
});
```

**Step 2: 跑测试确认失败**

```bash
cd src-tauri/frontend && pnpm test:run src/components/CompanionRoot.test.tsx
```

Expected: 新用例中 modifier 相关 3 条 FAIL（`startDraggingMock` 被调用，因为守卫还没实现）；direct/事件用例的行为部分通过但事件用例 FAIL（未订阅事件）。

**Step 3: 实现**

1. import（CompanionRoot.tsx:7-15）：`onCompanionLockedChanged` 后加 `onCompanionDragModeChanged`；类型 import（17 行）改为：

```typescript
import type { CompanionDragMode, CompanionWindowLayer } from "@/types/tauri";
```

2. state（56 行 `const [locked, setLocked] = useState(false);` 后）：

```typescript
  // 拖拽模式：modifier = 需按住 cmd/ctrl 才能拖动（缺省 direct = 直接拖动）。
  const [dragMode, setDragMode] = useState<CompanionDragMode>("direct");
```

3. config 恢复（137 行 `setLocked(config.locked ?? false);` 后）：

```typescript
    if (config.drag_mode) setDragMode(config.drag_mode);
```

4. 事件订阅（193 行 locked 的 useEffect 后）：

```typescript
  // 设置面板切换拖拽模式时同步（只影响 mousedown 拖动条件，不影响缩放与右键）。
  useEffect(() => {
    const unlisten = onCompanionDragModeChanged((m) => {
      setDragMode(m);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);
```

5. mousedown 守卫（261-262 行）：

```tsx
      onMouseDown={(e) => {
        if (e.button !== 0 || layer === "back" || locked) return;
        if (dragMode === "modifier" && !(e.metaKey || e.ctrlKey)) return;
        void getCurrentWindow().startDragging();
      }}
```

6. 组件 doc comment（42 行）`按住左键拖动移动窗口（位置锁定时禁止）` 改为 `按住左键拖动移动窗口（位置锁定时禁止；修饰键模式下需按住 cmd/ctrl）`。

**Step 4: 跑测试确认通过**

```bash
pnpm test:run src/components/CompanionRoot.test.tsx
```

Expected: 全部 PASS（含存量「位置锁定」describe）。

**Step 5: Commit**

```bash
git add src-tauri/frontend/src/components/CompanionRoot.tsx src-tauri/frontend/src/components/CompanionRoot.test.tsx
git commit -m "feat(ui): CompanionRoot 支持 modifier 拖拽模式（需按住 cmd/ctrl）"
```

---

### Task 5: 设置页开关（TDD）

**Files:**
- Modify: `src-tauri/frontend/src/pages/CompanionPage.tsx`
- Test: `src-tauri/frontend/src/pages/CompanionPage.test.tsx`

**Step 1: 写失败测试**

1. hoisted `configState`（test 文件 20 行）改为：

```typescript
  configState: {
    clickThrough: null as boolean | null,
    locked: null as boolean | null,
    dragMode: null as string | null,
  },
```

2. `beforeEach`（87 行 `configState.locked = null;` 后）加 `configState.dragMode = null;`；mock config（106 行 `locked: configState.locked,` 后）加 `drag_mode: configState.dragMode,`。

3. 「锁定位置」用例（602 行后）新增：

```tsx
  it("修饰键拖动开关默认关闭，点击后调用 set_companion_drag_mode 切到 modifier", async () => {
    library = { models: [MODEL_A], active_model_id: MODEL_A.id };
    const user = userEvent.setup();
    renderPage();

    const toggle = await screen.findByRole("switch", { name: "修饰键拖动" });
    expect(toggle).toHaveAttribute("aria-checked", "false");

    await user.click(toggle);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_companion_drag_mode", { mode: "modifier" });
    });
  });

  it("修饰键拖动开关从配置恢复为开启，再点击切回 direct", async () => {
    library = { models: [MODEL_A], active_model_id: MODEL_A.id };
    configState.dragMode = "modifier";
    const user = userEvent.setup();
    renderPage();

    const toggle = await screen.findByRole("switch", { name: "修饰键拖动" });
    expect(toggle).toHaveAttribute("aria-checked", "true");

    await user.click(toggle);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_companion_drag_mode", { mode: "direct" });
    });
  });

  it("未选中伙伴时修饰键拖动开关仍然可见（窗口级行为）", async () => {
    library = { models: [], active_model_id: null };
    renderPage();

    expect(await screen.findByRole("switch", { name: "修饰键拖动" })).toBeInTheDocument();
  });
```

**Step 2: 跑测试确认失败**

```bash
cd src-tauri/frontend && pnpm test:run src/pages/CompanionPage.test.tsx
```

Expected: 3 条新用例 FAIL——找不到 `switch` role name「修饰键拖动」。

**Step 3: 实现**

1. 类型 import（CompanionPage.tsx:30）改为：

```typescript
import type { CompanionDragMode, CompanionModelInfo, CompanionWindowLayer } from "@/types/tauri";
```

2. state（392 行 `const [locked, setLocked] = useState(false);` 后）：

```typescript
  const [dragMode, setDragMode] = useState<CompanionDragMode>("direct");
```

3. config 恢复（402 行 `setLocked(cfg.locked ?? false);` 后）：

```typescript
        setDragMode(cfg.drag_mode ?? "direct");
```

4. handler（428 行 `handleToggleLocked` 后）：

```typescript
  const handleToggleDragMode = useCallback((enabled: boolean) => {
    const next: CompanionDragMode = enabled ? "modifier" : "direct";
    setDragMode(next);
    void api.setCompanionDragMode({ mode: next });
  }, []);
```

5. Switch UI（600 行「位置锁定」div 的 `</div>` 后）：

```tsx
              {/* 拖拽模式（窗口级）：modifier = 需按住 cmd/Ctrl 才能拖动，与锁定正交（锁定优先） */}
              <div className="flex w-full items-center gap-2">
                <span className="shrink-0">修饰键拖动</span>
                <Switch
                  aria-label="修饰键拖动"
                  checked={dragMode === "modifier"}
                  onCheckedChange={handleToggleDragMode}
                />
                <span className="flex-1 text-xs text-muted-foreground">
                  开启后需按住 ⌘/Ctrl 才能拖动窗口，滚轮缩放与右键菜单不受影响
                </span>
              </div>
```

6. 387 行注释「显示层级（置顶/置底，窗口级）」后顺带补一行注释提及拖拽模式（可选，保持注释列表完整）。

**Step 4: 跑测试确认通过**

```bash
pnpm test:run src/pages/CompanionPage.test.tsx
```

Expected: 全部 PASS。

**Step 5: Commit**

```bash
git add src-tauri/frontend/src/pages/CompanionPage.tsx src-tauri/frontend/src/pages/CompanionPage.test.tsx
git commit -m "feat(ui): 设置页新增修饰键拖动开关（桌宠拖拽模式）"
```

---

### Task 6: 全量验证

**Step 1: Rust 全量**

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

Expected: 全绿（注意 `cargo test` 在本仓库直接跑即可；若 env 竞争失败改用 `cargo test -- --test-threads=1`）。

**Step 2: tauri crate 检查**

```bash
cargo check -p zapmomo-app && cargo clippy -p zapmomo-app -- -D warnings
```

Expected: 零告警。

**Step 3: 前端全量**

```bash
cd src-tauri/frontend && pnpm exec tsc -b && pnpm test:run && pnpm check
```

Expected: tsc 零错误、Vitest 全绿、biome 无问题。

**Step 4: 手动冒烟（可选，需 GUI）**

`pnpm tauri dev` → 设置页开「修饰键拖动」→ 桌宠裸拖不动、按住 ⌘ 可拖、cmd+滚轮缩放仍可用、锁定优先。

**Step 5: 无新增提交（验证任务）；如有修复则按所属文件补提交**
