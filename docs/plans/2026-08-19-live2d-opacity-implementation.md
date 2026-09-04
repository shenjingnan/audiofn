# Live2D 模型透明度调节 · 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 Live2D 桌宠增加模型透明度调节：右键菜单 / 托盘菜单（5 档 + 勾选态）与设置页 Slider（20%~100%），持久化并即时生效。

**Architecture:** 完全复刻 `window_scale` 的四层链路（settings.toml 字段 → Rust command 写盘 + emit 事件 → 前端绑定 → 渲染层应用）。差异点：透明度无 Tauri 窗口 API，在前端用 CSS opacity 应用于包裹 `Live2dStage` 的 wrapper div；后端只存数字 + 发事件。设计文档：`docs/plans/2026-08-19-live2d-opacity-design.md`。

**Tech Stack:** Rust (tauri 2 menu/CheckboxMenuItem, serde/TOML) + React 19 (Radix Slider) + Vitest。

**关键参考（现有 scale 实现）：**
- `src/config/settings.rs:372` — `window_scale` 字段与 serde 属性
- `src-tauri/src/lib.rs:2136-2166` — `apply_companion_scale` / `scale_from_id` / command
- `src-tauri/src/lib.rs:2202-2239` — `handle_menu` / `build_companion_menu`
- `src-tauri/src/lib.rs:3482` — 托盘菜单（当前无尺寸入口）
- `src-tauri/frontend/src/components/CompanionRoot.tsx:39,91-117,191-196`
- `src-tauri/frontend/src/pages/CompanionPage.tsx:264-278,383-397`
- `src-tauri/frontend/src/pages/CompanionPage.test.tsx:130-147`

**已踩过的坑（勿重复）：**
1. `Live2dSettings` 加字段只影响 `settings.rs:885,894` 两处**测试字面量**（其它构造处均 `..Default::default()`），需补 `window_opacity`。
2. `CompanionPage` 加第二个 Slider 后，现有测试的 `getByRole("slider")` 与 `findByText("100%")` 变成多匹配 → 必须给两个 Slider 加 `aria-label` 并更新断言。
3. `get_live2d_config` 里 `live2d_settings.and_then(...)` 是 move 语义，加第二个字段前先提取局部变量。
4. cargo test 用 `-- --test-threads=1`（CLAUDE.md：避免 env 竞争）。

---

### Task 0: 创建 feature 分支

**Step 1: 从 main 创建并切换分支**

```bash
git checkout -b feature/live2d-model-opacity
```

Expected: `Switched to a new branch 'feature/live2d-model-opacity'`

---

### Task 1: settings.rs 加 `window_opacity` 字段（TDD）

**Files:**
- Modify: `src/config/settings.rs:372`（字段）
- Test: `src/config/settings.rs:885,894`（现有 roundtrip 用例补字段 + 断言）

**Step 1: 改现有测试使其失败**

`test_live2d_settings_serde_roundtrip`（settings.rs:884）两处字面量加字段：

```rust
let live2d = Live2dSettings {
    model_dir: Some("/tmp/some-model".to_string()),
    window_position: Some(CompanionWindowPosition { x: 120, y: 800 }),
    window_scale: Some(1.5),
    window_opacity: Some(0.6),
};
```

```rust
let none_pos = Live2dSettings {
    model_dir: Some("/tmp/some-model".to_string()),
    window_position: None,
    window_scale: None,
    window_opacity: None,
};
```

并在 `none_toml` 断言区补一行：

```rust
assert!(!none_toml.contains("window_opacity"));
```

**Step 2: 跑测试确认编译失败（字段不存在）**

```bash
cargo test --lib config::settings::tests::test_live2d_settings_serde_roundtrip -- --test-threads=1
```

Expected: FAIL — `no field 'window_opacity' on type 'Live2dSettings'`

**Step 3: 加字段**

`Live2dSettings`（settings.rs:372 `window_scale` 之后）：

```rust
/// 角色窗口透明度（1.0 = 不透明；缺省视为 1.0）
#[serde(default, skip_serializing_if = "Option::is_none")]
pub window_opacity: Option<f64>,
```

**Step 4: 跑测试确认通过**

```bash
cargo test --lib config::settings::tests::test_live2d_settings_serde_roundtrip -- --test-threads=1
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/config/settings.rs
git commit -m "feat(live2d): settings 增加 window_opacity 透明度配置字段"
```

---

### Task 2: lib.rs 纯函数 `clamp_opacity` / `opacity_from_id` + 单测（TDD）

**Files:**
- Modify: `src-tauri/src/lib.rs`（`scale_from_id` 之后 ~2157 行；文件底部新增 tests 模块）

**Step 1: 写失败测试**

在 `src-tauri/src/lib.rs` **文件末尾**新增（该文件目前无 tests 模块）：

```rust
#[cfg(test)]
mod companion_opacity_tests {
    use super::{clamp_opacity, opacity_from_id};

    #[test]
    fn test_opacity_from_id_mappings() {
        assert_eq!(opacity_from_id("opacity_100"), Some(1.0));
        assert_eq!(opacity_from_id("opacity_80"), Some(0.8));
        assert_eq!(opacity_from_id("opacity_60"), Some(0.6));
        assert_eq!(opacity_from_id("opacity_40"), Some(0.4));
        assert_eq!(opacity_from_id("opacity_20"), Some(0.2));
        assert_eq!(opacity_from_id("scale_100"), None);
        assert_eq!(opacity_from_id("unknown"), None);
    }

    #[test]
    fn test_clamp_opacity_bounds() {
        assert_eq!(clamp_opacity(0.05), 0.2);
        assert_eq!(clamp_opacity(-1.0), 0.2);
        assert_eq!(clamp_opacity(1.5), 1.0);
        assert_eq!(clamp_opacity(0.2), 0.2);
        assert_eq!(clamp_opacity(1.0), 1.0);
        assert_eq!(clamp_opacity(0.65), 0.65);
    }
}
```

**Step 2: 跑测试确认失败**

```bash
cargo test -p zapmomo-app companion_opacity -- --test-threads=1
```

Expected: FAIL — `cannot find function 'clamp_opacity' / 'opacity_from_id'`

**Step 3: 实现（`scale_from_id` 函数之后插入）**

```rust
/// 透明度合法范围（含边界）。
const OPACITY_MIN: f64 = 0.2;
const OPACITY_MAX: f64 = 1.0;

/// 把透明度 clamp 到 `[OPACITY_MIN, OPACITY_MAX]`。
fn clamp_opacity(v: f64) -> f64 {
    v.clamp(OPACITY_MIN, OPACITY_MAX)
}

/// 把原生菜单项 id 解析为透明度。
fn opacity_from_id(id: &str) -> Option<f64> {
    match id {
        "opacity_100" => Some(1.0),
        "opacity_80" => Some(0.8),
        "opacity_60" => Some(0.6),
        "opacity_40" => Some(0.4),
        "opacity_20" => Some(0.2),
        _ => None,
    }
}
```

**Step 4: 跑测试确认通过**

```bash
cargo test -p zapmomo-app companion_opacity -- --test-threads=1
```

Expected: PASS（2 个用例）

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(live2d): 透明度档位 id 映射与 clamp 纯函数"
```

---

### Task 3: lib.rs `apply_companion_opacity` + command + config 字段

**Files:**
- Modify: `src-tauri/src/lib.rs:1842`（`Live2dConfigInfo`）、`1868`（填充）、`2136` 附近（apply/command）、`3255`（invoke_handler 注册）

**Step 1: `apply_companion_scale`（lib.rs:2143）之后插入**

```rust
/// 保存角色窗口透明度并通知角色窗口（内部实现，供 command 与原生菜单事件共用）。
fn apply_companion_opacity(app: &AppHandle, opacity: f64) -> Result<(), String> {
    let opacity = clamp_opacity(opacity);
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let live2d = settings.live2d.get_or_insert_with(Live2dSettings::default);
    live2d.window_opacity = Some(opacity);
    settings::save_settings(&settings)?;
    let _ = app.emit("companion-opacity-changed", opacity);
    Ok(())
}
```

**Step 2: `set_companion_scale` command（lib.rs:2166）之后插入**

```rust
/// 设置并持久化角色窗口透明度（1.0 = 不透明，范围 0.2~1.0）。
///
/// 由设置面板调用：写入 `~/.zapmomo/settings.toml` 的 `[live2d].window_opacity`，
/// 并通过 `companion-opacity-changed` 事件通知角色窗口更新渲染层 opacity。
#[tauri::command]
fn set_companion_opacity(app: AppHandle, opacity: f64) -> Result<(), String> {
    apply_companion_opacity(&app, opacity)
}
```

**Step 3: `Live2dConfigInfo`（lib.rs:1842）加字段**

```rust
window_scale: Option<f64>,
window_opacity: Option<f64>,
```

**Step 4: `get_live2d_config` 填充（lib.rs:1863-1870）**

把 `window_scale: live2d_settings.and_then(|l| l.window_scale),` 替换为**先提取再使用**（move 语义，见坑 3）：

```rust
let window_scale = live2d_settings.as_ref().and_then(|l| l.window_scale);
let window_opacity = live2d_settings.as_ref().and_then(|l| l.window_opacity);

Ok(Live2dConfigInfo {
    model_dir: Some(cfg.model_dir.display().to_string()),
    model_file: cfg.model_file.map(|p| p.display().to_string()),
    format: cfg.format.map(|f| f.to_str().to_string()),
    models_present,
    window_scale,
    window_opacity,
    settings_path: settings::get_settings_path().display().to_string(),
})
```

**Step 5: 注册 command（lib.rs:3255 `set_companion_scale,` 之后）**

```rust
set_companion_opacity,
```

**Step 6: 编译 + 测试**

```bash
cargo test -p zapmomo-app -- --test-threads=1 && cargo clippy -p zapmomo-app -- -D warnings
```

Expected: 全部 PASS，无 warning

**Step 7: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(live2d): set_companion_opacity 命令与配置下发"
```

---

### Task 4: 菜单（右键 + 托盘 + handle_menu 分支，CheckboxMenuItem 勾选态）

**Files:**
- Modify: `src-tauri/src/lib.rs:14`（import）、`2202-2239`（handle_menu + build_companion_menu）、`3475-3483`（托盘菜单）

**Step 1: import 加 CheckboxMenuItem（lib.rs:14）**

```rust
use tauri::menu::{CheckboxMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
```

**Step 2: 读当前值的 helper + 共用子菜单构建（插在 `build_companion_menu` 之前）**

```rust
/// 读当前窗口缩放与透明度（读失败或缺省回退 1.0 / 1.0）。
fn current_companion_metrics() -> (f64, f64) {
    match settings::load_settings() {
        Ok(Some(s)) => {
            let live2d = s.live2d.as_ref();
            (
                live2d.and_then(|l| l.window_scale).unwrap_or(1.0),
                live2d.and_then(|l| l.window_opacity).unwrap_or(1.0),
            )
        }
        _ => (1.0, 1.0),
    }
}

/// 构建「窗口尺寸」「透明度」两个档位子菜单（角色右键菜单与托盘菜单共用）。
///
/// 档位用 `CheckboxMenuItem`：构建时读当前 settings，命中的档位打勾。
fn build_metric_submenus(
    app: &AppHandle,
) -> tauri::Result<(Submenu<tauri::Wry>, Submenu<tauri::Wry>)> {
    let (cur_scale, cur_opacity) = current_companion_metrics();
    let mk_item = |id: &str, label: &str, cur: f64, v: f64| {
        CheckboxMenuItem::with_id(app, id, label, true, v == cur, None::<&str>)
    };
    let s25 = mk_item("scale_25", "25%", cur_scale, 0.25)?;
    let s50 = mk_item("scale_50", "50%", cur_scale, 0.5)?;
    let s70 = mk_item("scale_70", "70%", cur_scale, 0.7)?;
    let s100 = mk_item("scale_100", "100%", cur_scale, 1.0)?;
    let s150 = mk_item("scale_150", "150%", cur_scale, 1.5)?;
    let s200 = mk_item("scale_200", "200%", cur_scale, 2.0)?;
    let o100 = mk_item("opacity_100", "100%", cur_opacity, 1.0)?;
    let o80 = mk_item("opacity_80", "80%", cur_opacity, 0.8)?;
    let o60 = mk_item("opacity_60", "60%", cur_opacity, 0.6)?;
    let o40 = mk_item("opacity_40", "40%", cur_opacity, 0.4)?;
    let o20 = mk_item("opacity_20", "20%", cur_opacity, 0.2)?;
    let scale_menu = Submenu::with_items(
        app,
        "窗口尺寸",
        true,
        &[&s25, &s50, &s70, &s100, &s150, &s200],
    )?;
    let opacity_menu =
        Submenu::with_items(app, "透明度", true, &[&o100, &o80, &o60, &o40, &o20])?;
    Ok((scale_menu, opacity_menu))
}
```

**Step 3: 重写 `build_companion_menu`（lib.rs:2217-2239）**

```rust
/// 构建角色窗口的右键菜单（窗口尺寸/透明度子菜单 + 打开设置 / 隐藏角色 / 重启 / 退出）。
fn build_companion_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let (scale_submenu, opacity_submenu) = build_metric_submenus(app)?;
    let open_settings = MenuItem::with_id(app, "open_settings", "打开设置", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide_companion", "隐藏角色", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, "restart", "重启", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    Menu::with_items(
        app,
        &[
            &scale_submenu,
            &opacity_submenu,
            &open_settings,
            &hide,
            &restart,
            &quit,
        ],
    )
}
```

**Step 4: `handle_menu` fallback 分支（lib.rs:2209-2213）**

```rust
_ => {
    if let Some(scale) = scale_from_id(id) {
        let _ = apply_companion_scale(app, scale);
    } else if let Some(opacity) = opacity_from_id(id) {
        let _ = apply_companion_opacity(app, opacity);
    }
}
```

**Step 5: 托盘菜单（lib.rs:3475-3483）加两个子菜单**

在 `let tray_menu = ...` 之前插入子菜单构建，并把它加进 items：

```rust
// 托盘菜单：显示/隐藏角色、窗口尺寸/透明度、打开设置、重启、退出。
let (tray_scale, tray_opacity) = build_metric_submenus(app)?;
let toggle_companion =
    MenuItem::with_id(app, "toggle_companion", "显示/隐藏角色", true, None::<&str>)?;
let open_settings =
    MenuItem::with_id(app, "open_settings", "打开设置", true, None::<&str>)?;
let restart = MenuItem::with_id(app, "restart", "重启", true, None::<&str>)?;
let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
let tray_menu = Menu::with_items(
    app,
    &[
        &toggle_companion,
        &tray_scale,
        &tray_opacity,
        &open_settings,
        &restart,
        &quit,
    ],
)?;
```

（注释一并更新为「显示/隐藏角色、窗口尺寸/透明度、打开设置、重启、退出」。）

**Step 6: 编译 + Lint**

```bash
cargo clippy -p zapmomo-app -- -D warnings && cargo test -p zapmomo-app -- --test-threads=1
```

Expected: 无 warning，测试全 PASS

**Step 7: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(live2d): 右键/托盘菜单透明度档位（勾选态），托盘补齐窗口尺寸入口"
```

---

### Task 5: 前端类型与绑定

**Files:**
- Modify: `src-tauri/frontend/src/types/tauri.ts:118`、`src-tauri/frontend/src/lib/tauri.ts:173,246`

**Step 1: `Live2dConfigInfo`（types/tauri.ts:118 `window_scale` 之后）**

```ts
window_opacity: number | null;
```

**Step 2: api 对象（lib/tauri.ts:173 `setCompanionScale` 之后）**

```ts
setCompanionOpacity: (args: { opacity: number }) => invoke<void>("set_companion_opacity", args),
```

**Step 3: 事件订阅（lib/tauri.ts:246 `onCompanionScaleChanged` 之后）**

```ts
export function onCompanionOpacityChanged(handler: (opacity: number) => void): Promise<UnlistenFn> {
  return listen<number>("companion-opacity-changed", (e) => handler(e.payload));
}
```

**Step 4: 类型检查**

```bash
cd src-tauri/frontend && pnpm build
```

Expected: `tsc -b` + vite build 通过（此时无消费者，仅绑定就绪）

**Step 5: Commit**

```bash
git add src-tauri/frontend/src/types/tauri.ts src-tauri/frontend/src/lib/tauri.ts
git commit -m "feat(live2d): 前端透明度 command 绑定与事件订阅"
```

---

### Task 6: CompanionRoot 应用透明度

**Files:**
- Modify: `src-tauri/frontend/src/components/CompanionRoot.tsx:7,27-29,39,91-96,191-196`

**Step 1: import（第 7 行）加 `onCompanionOpacityChanged`**

```tsx
import { api, onCompanionOpacityChanged, onCompanionScaleChanged, onLive2dModelChanged, toAssetUrl } from "@/lib/tauri";
```

**Step 2: state（第 39 行 `scale` 之后）**

```tsx
const [opacity, setOpacity] = useState(1.0);
```

**Step 3: 初始化（91-96 的 useEffect 内，读 config 处一并恢复透明度）**

```tsx
// 恢复持久化的缩放比例与透明度，并据此 resize 一次（确保前端 state 与后端建窗尺寸一致）。
useEffect(() => {
  if (!config) return;
  const s = config.window_scale ?? 1.0;
  setScale(s);
  setOpacity(config.window_opacity ?? 1.0);
  void resizeTo(aspectRatioRef.current, s);
}, [config, resizeTo]);
```

**Step 4: 事件监听（117 行 scale 监听 useEffect 之后新增）**

```tsx
// 设置面板/菜单改透明度时同步（纯视觉：只更新渲染层 opacity，不涉及窗口尺寸）。
useEffect(() => {
  const unlisten = onCompanionOpacityChanged((v) => {
    setOpacity(v);
  });
  return () => {
    void unlisten.then((fn) => fn());
  };
}, []);
```

**Step 5: 应用到渲染（191-196 的 `<Live2dStage>` 包 wrapper；VoiceStatusDot 留在 wrapper 外保持不透明）**

```tsx
{/* 透明度只作用于模型本身，语音状态点保持不透明 */}
<div style={{ opacity }}>
  <Live2dStage
    modelUrl={modelUrl}
    width={size.width}
    height={size.height}
    onModelMetrics={handleModelMetrics}
  />
</div>
```

**Step 6: 更新组件头注释（27-29 行）**

在「订阅 `live2d-model-changed` / `companion-scale-changed`」一句中补上 `companion-opacity-changed`，并注明透明度由 wrapper div 的 `style.opacity` 应用。

**Step 7: 构建 + Lint**

```bash
cd src-tauri/frontend && pnpm build && pnpm check
```

Expected: 通过

**Step 8: Commit**

```bash
git add src-tauri/frontend/src/components/CompanionRoot.tsx
git commit -m "feat(live2d): 角色窗口应用透明度（CSS opacity，不含状态点）"
```

---

### Task 7: 设置页透明度 Slider（TDD）

**Files:**
- Modify: `src-tauri/frontend/src/pages/CompanionPage.tsx:264-278,383-397`
- Test: `src-tauri/frontend/src/pages/CompanionPage.test.tsx:64-72,130-147`

**Step 1: 改测试（先失败）**

1. mock `get_live2d_config` 返回对象（64-72 行）加 `window_opacity: 1.0,`
2. 现有 scale 用例（130-147）更新断言（坑 2：双 Slider 多匹配）：

```tsx
it("选中伙伴后显示桌宠尺寸控制，拖动滑块调用 set_companion_scale", async () => {
  library = { models: [MODEL_A], active_model_id: MODEL_A.id };
  const user = userEvent.setup();
  renderPage();

  await screen.findByRole("button", { name: /大月下.*使用中/ });
  // 初始从 get_live2d_config 读到 window_scale=1.0 → 100%（异步等待出现）。
  expect(await screen.findByText("桌宠尺寸")).toBeInTheDocument();
  // 尺寸与透明度两个滑块初始都是 100%。
  expect(await screen.findAllByText("100%")).toHaveLength(2);

  // 键盘微调滑块（Radix Slider role="slider"）：每次步进 5。
  const slider = screen.getByRole("slider", { name: "桌宠尺寸" });
  slider.focus();
  await user.keyboard("{ArrowRight}");
  await waitFor(() => {
    expect(invokeMock).toHaveBeenCalledWith("set_companion_scale", { scale: expect.any(Number) });
  });
});
```

3. 该 describe 块内新增用例：

```tsx
it("选中伙伴后显示透明度控制，拖动滑块调用 set_companion_opacity", async () => {
  library = { models: [MODEL_A], active_model_id: MODEL_A.id };
  const user = userEvent.setup();
  renderPage();

  await screen.findByRole("button", { name: /大月下.*使用中/ });
  expect(await screen.findByText("透明度")).toBeInTheDocument();
  expect(await screen.findAllByText("100%")).toHaveLength(2);

  const slider = screen.getByRole("slider", { name: "透明度" });
  slider.focus();
  await user.keyboard("{ArrowLeft}"); // 100 → 95
  await waitFor(() => {
    expect(invokeMock).toHaveBeenCalledWith("set_companion_opacity", {
      opacity: expect.any(Number),
    });
  });
});
```

**Step 2: 跑测试确认失败**

```bash
cd src-tauri/frontend && pnpm test:run -- CompanionPage
```

Expected: 2 个用例 FAIL（找不到 name="桌宠尺寸"/"透明度" 的 slider、找不到 "透明度" 文本）

**Step 3: 实现设置页**

1. state + 初始化（264-278，合并读 config）：

```tsx
// 桌宠尺寸/透明度（百分比）：写入 settings 并通知桌宠窗口即时生效。
const [percent, setPercent] = useState(100);
const [opacityPercent, setOpacityPercent] = useState(100);
useEffect(() => {
  void api
    .getLive2dConfig()
    .then((cfg) => {
      if (cfg.window_scale != null) setPercent(Math.round(cfg.window_scale * 100));
      if (cfg.window_opacity != null) setOpacityPercent(Math.round(cfg.window_opacity * 100));
    })
    .catch(() => {});
}, []);
const handleScaleChange = useCallback((value: number) => {
  const clamped = Math.max(25, Math.min(200, Math.round(value)));
  setPercent(clamped);
  void api.setCompanionScale({ scale: clamped / 100 });
}, []);
const handleOpacityChange = useCallback((value: number) => {
  const clamped = Math.max(20, Math.min(100, Math.round(value)));
  setOpacityPercent(clamped);
  void api.setCompanionOpacity({ opacity: clamped / 100 });
}, []);
```

2. UI（383-397 区域，两个控件并排；现有 Slider 补 `aria-label="桌宠尺寸"`）：

```tsx
{/* 桌宠尺寸/透明度：调整窗口缩放与模型透明度，同步到桌宠窗口 */}
{selected && (
  <div className="flex items-center gap-4 text-sm text-muted-foreground">
    <div className="flex items-center gap-2">
      <span className="shrink-0">桌宠尺寸</span>
      <Slider
        aria-label="桌宠尺寸"
        value={[percent]}
        min={25}
        max={200}
        step={5}
        onValueChange={([v]) => handleScaleChange(v)}
        className="w-28"
      />
      <span className="w-10 shrink-0 text-right tabular-nums">{percent}%</span>
    </div>
    <div className="flex items-center gap-2">
      <span className="shrink-0">透明度</span>
      <Slider
        aria-label="透明度"
        value={[opacityPercent]}
        min={20}
        max={100}
        step={5}
        onValueChange={([v]) => handleOpacityChange(v)}
        className="w-28"
      />
      <span className="w-10 shrink-0 text-right tabular-nums">{opacityPercent}%</span>
    </div>
  </div>
)}
```

**Step 4: 跑测试确认通过**

```bash
cd src-tauri/frontend && pnpm test:run -- CompanionPage
```

Expected: 全 PASS（含原有其余用例）

**Step 5: Commit**

```bash
git add src-tauri/frontend/src/pages/CompanionPage.tsx src-tauri/frontend/src/pages/CompanionPage.test.tsx
git commit -m "feat(live2d): 设置页透明度滑块（20%~100%）"
```

---

### Task 8: 全量验收

**Step 1: Rust 完整检查（根目录）**

```bash
cargo fmt && cargo fmt --check && cargo clippy -- -D warnings && cargo clippy -p zapmomo-app -- -D warnings && cargo test -- --test-threads=1
```

Expected: 全部通过（fmt 已格式化无 diff、clippy 无 warning、测试全绿）

**Step 2: 前端完整检查**

```bash
cd src-tauri/frontend && pnpm build && pnpm check && pnpm test:run
```

Expected: tsc/vite build、biome、vitest 全绿

**Step 3: 手动验收（`pnpm tauri dev`，交由用户确认）**

1. 右键模型 → 「透明度」子菜单 5 档，当前档打勾；切换后模型立即变化。
2. 托盘菜单 → 「窗口尺寸」「透明度」两个子菜单均可用且打勾正确。
3. 设置页「透明度」Slider 拖动 → 模型即时变化；「桌宠尺寸」原有行为不回归。
4. 调低透明度后重启应用 → 透明度保持。
5. cmd/ctrl+滚轮缩放（scale 现有交互）不受影响；语音状态点始终不透明。

**Step 4: 收尾 Commit（如有 fmt 产生的改动）**

```bash
git add -A && git commit -m "style: cargo fmt"
```
