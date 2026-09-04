# 伙伴页 Live2D 动作/表情预览 — 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 伙伴页可枚举并点击预览 Live2D 模型的动作（Motion）与表情（Expression）。

**Architecture:** 三阶段——Rust 导入器把磁盘上未注册的 `*.motion3.json` / `*.exp3.json` 回写进托管副本 model3.json（统一 `Extra` 组，避开会自动循环播放的 `Idle` 组）并对存量模型做一次性迁移；前端 `PreviewManager` 暴露能力枚举回调与播放/重置方法；伙伴页渲染动作/表情面板。

**Tech Stack:** Rust（serde_json 无类型读-改-写 + 原子写）、React 19 + TypeScript、pixi-live2d-display 0.4.0（cubism4）、Vitest。

**设计文档:** `docs/plans/2026-08-20-companion-motion-preview-design.md`

**Worktree:** `.worktrees/analyze-partner-live2d-motions`（分支重命名为 `feature/companion-motion-preview`，见 Task 0）

---

## Task 0: 分支与基线

**Step 1: 重命名分支**（当前 `analyze/partner-live2d-motions` → 项目规范的 feature 前缀）

```bash
cd /Users/nemo/Projects/shenjingnan/zapmomo/.worktrees/analyze-partner-live2d-motions
git branch -m feature/companion-motion-preview
```

**Step 2: 确认基线绿**

```bash
cargo test --quiet 2>&1 | tail -5
```

Expected: 全部通过（main 基线干净）。

---

## Task 1: Rust — 补注册核心函数 `register_missing_motion_files`

**Files:**
- Modify: `src/companion.rs`（实现 + 测试，测试在文件末尾 `mod tests` 内追加）

**Step 1: 写失败测试**（`src/companion.rs` tests 模块内，`make_valid_model` fixture 之后追加）

```rust
    /// 在 fixture 基础上补未注册的动作/表情文件（模拟「火花」这类模型：文件在、清单没登记）。
    fn add_unregistered_assets(dir: &Path) {
        std::fs::create_dir_all(dir.join("Motions")).unwrap();
        std::fs::write(dir.join("Motions/睡觉动画.motion3.json"), "{}").unwrap();
        std::fs::write(dir.join("chibang.motion3.json"), "{}").unwrap(); // 根目录散文件
        std::fs::create_dir_all(dir.join("Expressions")).unwrap();
        std::fs::write(dir.join("Expressions/03 生气.exp3.json"), "{}").unwrap();
        std::fs::write(dir.join("Expressions/07 星星眼.exp3.json"), "{}").unwrap();
    }

    /// 解析 model3.json 为 Value（测试辅助）。
    fn read_manifest(model_file: &Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(model_file).unwrap()).unwrap()
    }

    #[test]
    fn test_register_missing_motion_files_registers_extra_and_expressions() {
        run_with_temp_home(|home| {
            let src = home.join("m");
            make_valid_model(&src, "m.model3.json");
            add_unregistered_assets(&src);
            let model_file = src.join("m.model3.json");

            assert!(register_missing_motion_files(&model_file).unwrap());

            let json = read_manifest(&model_file);
            let refs = json.get("FileReferences").unwrap();
            // 两个动作都进 Extra 组，路径为相对 model3.json 的正斜杠相对路径。
            let extra = refs
                .get("Motions").and_then(|m| m.get("Extra")).and_then(|e| e.as_array()).unwrap();
            let files: Vec<&str> = extra.iter().filter_map(|e| e.get("File").and_then(|f| f.as_str())).collect();
            assert_eq!(files, vec!["Motions/睡觉动画.motion3.json", "chibang.motion3.json"]);
            // 绝不写 Idle 组（库会对 Idle 自动循环播放，改变桌宠静态行为）。
            assert!(refs.get("Motions").and_then(|m| m.get("Idle")).is_none());
            // 表情注册，Name 为文件名去扩展名。
            let exprs = refs.get("Expressions").and_then(|e| e.as_array()).unwrap();
            assert_eq!(exprs.len(), 2);
            assert_eq!(exprs[0].get("Name").and_then(|n| n.as_str()), Some("07 星星眼"));
            assert_eq!(exprs[1].get("Name").and_then(|n| n.as_str()), Some("03 生气"));
            // Moc/Textures 原样保留。
            assert_eq!(refs.get("Moc").and_then(|v| v.as_str()), Some("model.moc3"));
            // 写后仍可通过校验。
            live2d_cfg::validate_managed_model(&src).unwrap();
        });
    }

    #[test]
    fn test_register_missing_motion_files_idempotent_and_skips_registered() {
        run_with_temp_home(|home| {
            let src = home.join("m");
            // 已有注册：Idle 组一个动作 + 一个表情（作者自己写的，绝不能动）。
            std::fs::create_dir_all(src.join("textures")).unwrap();
            std::fs::write(src.join("model.moc3"), b"moc").unwrap();
            std::fs::write(src.join("textures/texture_00.png"), b"png").unwrap();
            std::fs::write(
                src.join("m.model3.json"),
                r#"{"FileReferences":{
                    "Moc":"model.moc3",
                    "Textures":["textures/texture_00.png"],
                    "Motions":{"Idle":[{"File":"idle.motion3.json"}]},
                    "Expressions":[{"Name":"开心","File":"happy.exp3.json"}]}}"#,
            )
            .unwrap();
            std::fs::write(src.join("idle.motion3.json"), "{}").unwrap();
            std::fs::write(src.join("happy.exp3.json"), "{}").unwrap();
            let model_file = src.join("m.model3.json");
            let before = std::fs::read_to_string(&model_file).unwrap();

            // 已全部注册 → 无修改。
            assert!(!register_missing_motion_files(&model_file).unwrap());
            assert_eq!(std::fs::read_to_string(&model_file).unwrap(), before);

            // 注册后再调用 → 幂等。
            std::fs::write(src.join("new.motion3.json"), "{}").unwrap();
            assert!(register_missing_motion_files(&model_file).unwrap());
            assert!(!register_missing_motion_files(&model_file).unwrap());
            let json = read_manifest(&model_file);
            let refs = json.get("FileReferences").unwrap();
            // 作者的 Idle 组原样（未被追加），新动作进 Extra。
            let idle = refs.get("Motions").and_then(|m| m.get("Idle")).and_then(|e| e.as_array()).unwrap();
            assert_eq!(idle.len(), 1);
            let extra = refs.get("Motions").and_then(|m| m.get("Extra")).and_then(|e| e.as_array()).unwrap();
            assert_eq!(extra.len(), 1);
        });
    }

    #[test]
    fn test_register_missing_motion_files_rejects_bad_json() {
        run_with_temp_home(|home| {
            let src = home.join("m");
            make_valid_model(&src, "m.model3.json");
            std::fs::write(src.join("m.model3.json"), "{not-json").unwrap();
            let err = register_missing_motion_files(&src.join("m.model3.json")).unwrap_err();
            assert!(err.contains("解析"), "{err}");
            // 原文件未被覆盖。
            assert_eq!(std::fs::read_to_string(src.join("m.model3.json")).unwrap(), "{not-json");
        });
    }
```

**Step 2: 运行确认失败**

```bash
cargo test --quiet register_missing 2>&1 | tail -5
```

Expected: 编译失败 `cannot find function register_missing_motion_files`。

**Step 3: 实现**（`src/companion.rs`，放在 `save_cover` 之后、`MAX_NAME_CHARS` 常量之前的区域；常量加在文件顶部常量区）

顶部常量区（`const ID_PREFIX` 之后）加：

```rust
/// 补注册动作的统一组名。**绝不写 "Idle"**：pixi-live2d-display 对 Idle 组自动循环
/// 播放（空闲时每帧随机挑动作），写入会改变桌宠「静态展示」行为。
const EXTRA_MOTION_GROUP: &str = "Extra";
/// 存量伙伴补注册迁移标记（写入 `completed_migrations` 闸门，防重复执行）。
const MOTION_REGISTRATION_MIGRATION: &str = "motion-registration-v1";
```

实现函数（`use` 区需加 `std::collections::HashSet`）：

```rust
/// 递归扫描 dir 下所有 `*.motion3.json` / `*.exp3.json`，收集为相对 base 的
/// 正斜杠路径（Live2D 清单惯例，Windows 分隔符归一）。
fn collect_motion_assets(
    base: &Path,
    dir: &Path,
    motions: &mut Vec<String>,
    expressions: &mut Vec<String>,
) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("读取目录失败 ({}): {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_motion_assets(base, &path, motions, expressions)?;
            continue;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string());
        let rel = path.strip_prefix(base).ok().map(|p| p.to_string_lossy().replace('\\', "/"));
        let (Some(_), Some(rel)) = (name, rel) else {
            continue;
        };
        if rel.ends_with(".motion3.json") {
            motions.push(rel);
        } else if rel.ends_with(".exp3.json") {
            expressions.push(rel);
        }
    }
    Ok(())
}

/// 收集清单中已注册的 File 引用（Motions 各组并集 + Expressions）。
fn collect_registered_files(json: &serde_json::Value) -> HashSet<String> {
    let mut set = HashSet::new();
    let Some(refs) = json.get("FileReferences") else {
        return set;
    };
    if let Some(groups) = refs.get("Motions").and_then(|v| v.as_object()) {
        for motions in groups.values().filter_map(|v| v.as_array()) {
            for m in motions {
                if let Some(f) = m.get("File").and_then(|v| v.as_str()) {
                    set.insert(f.to_string());
                }
            }
        }
    }
    for e in refs.get("Expressions").and_then(|v| v.as_array()).into_iter().flatten() {
        if let Some(f) = e.get("File").and_then(|v| v.as_str()) {
            set.insert(f.to_string());
        }
    }
    set
}

/// 相对路径转展示名：basename 去掉 `.motion3.json` / `.exp3.json` 扩展。
fn asset_display_name(rel: &str) -> String {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.strip_suffix(".exp3.json")
        .or_else(|| name.strip_suffix(".motion3.json"))
        .unwrap_or(name)
        .to_string()
}

/// 原子写文件（tmp + rename，Windows 先移除再 rename 兜底；模式同 `save_library_inner`）。
fn atomic_write_file(path: &Path, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp-reg");
    std::fs::write(&tmp, contents).map_err(|e| format!("写入临时文件失败: {e}"))?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows：rename 无法覆盖已存在目标，先移除再重试。
            if path.exists() {
                std::fs::remove_file(path).map_err(|e| format!("移除旧文件失败: {e}"))?;
            }
            std::fs::rename(&tmp, path).map_err(|e| format!("原子替换失败: {e}"))
        }
    }
}

/// 扫描 model3.json 所在目录（含子目录）中存在但未注册的 `*.motion3.json` /
/// `*.exp3.json`，补注册进 FileReferences 后原子回写。返回是否发生了修改。
///
/// - 未注册动作统一进 `Motions["Extra"]`（不写 "Idle"，见 `EXTRA_MOTION_GROUP`）；
///   清单已有的组（含作者的 Idle）一律不动。
/// - 未注册表情进 `Expressions`，Name 取文件名去扩展名（允许重名：播放走 index）。
/// - 回写后重跑轻量校验，失败恢复原内容并按「无修改」返回（best-effort 增强，
///   注册失败不阻塞导入）。
fn register_missing_motion_files(model_file: &Path) -> Result<bool, String> {
    let Some(base) = model_file.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Err("模型清单缺少父目录".to_string());
    };
    let original = std::fs::read_to_string(model_file)
        .map_err(|e| format!("读取模型清单失败: {e}"))?;
    let mut json: serde_json::Value =
        serde_json::from_str(&original).map_err(|e| format!("解析模型清单失败: {e}"))?;

    let mut motions: Vec<String> = Vec::new();
    let mut expressions: Vec<String> = Vec::new();
    collect_motion_assets(base, base, &mut motions, &mut expressions)?;
    motions.sort();
    expressions.sort();

    let registered = collect_registered_files(&json);
    let new_motions: Vec<String> = motions
        .into_iter()
        .filter(|f| !registered.contains(f.as_str()))
        .collect();
    let new_expressions: Vec<String> = expressions
        .into_iter()
        .filter(|f| !registered.contains(f.as_str()))
        .collect();
    if new_motions.is_empty() && new_expressions.is_empty() {
        return Ok(false);
    }

    // 读-改-写（Value 无类型解析保留未知字段，serde_json::Map::entry 自动建缺失键）。
    let obj = json
        .as_object_mut()
        .ok_or_else(|| "模型清单不是 JSON 对象".to_string())?;
    let refs = obj
        .entry("FileReferences")
        .or_insert(serde_json::Value::Object(serde_json::Map::new()));
    let refs = refs
        .as_object_mut()
        .ok_or_else(|| "FileReferences 不是对象".to_string())?;
    if !new_motions.is_empty() {
        let groups = refs
            .entry("Motions")
            .or_insert(serde_json::Value::Object(serde_json::Map::new()));
        let groups = groups
            .as_object_mut()
            .ok_or_else(|| "Motions 不是对象".to_string())?;
        let extra = groups
            .entry(EXTRA_MOTION_GROUP)
            .or_insert(serde_json::Value::Array(Vec::new()));
        let extra = extra
            .as_array_mut()
            .ok_or_else(|| "Motions[Extra] 不是数组".to_string())?;
        for file in new_motions {
            extra.push(serde_json::json!({ "File": file }));
        }
    }
    if !new_expressions.is_empty() {
        let list = refs
            .entry("Expressions")
            .or_insert(serde_json::Value::Array(Vec::new()));
        let list = list
            .as_array_mut()
            .ok_or_else(|| "Expressions 不是数组".to_string())?;
        for file in new_expressions {
            let name = asset_display_name(&file);
            list.push(serde_json::json!({ "Name": name, "File": file }));
        }
    }

    let rewritten = serde_json::to_string_pretty(&json)
        .map_err(|e| format!("序列化模型清单失败: {e}"))?;
    atomic_write_file(model_file, &rewritten)?;

    // 写后二次校验：失败恢复原内容（不能弄坏一个本来可导入的模型）。
    if let Err(e) = live2d_cfg::validate_managed_model(base) {
        tracing::warn!("补注册后模型校验失败，已恢复原清单: {e}");
        let _ = atomic_write_file(model_file, &original);
        return Ok(false);
    }
    Ok(true)
}
```

**Step 4: 运行测试通过**

```bash
cargo test --quiet register_missing 2>&1 | tail -3
```

Expected: 3 个新测试 PASS。

**Step 5: fmt + clippy**

```bash
cargo fmt && cargo clippy --quiet -- -D warnings 2>&1 | tail -3
```

Expected: 无警告。

**Step 6: Commit**

```bash
git add src/companion.rs
git commit -m "feat(companion): 补注册托管副本中未登记的动作/表情文件"
```

---

## Task 2: Rust — `prepare_import` 接入补注册

**Files:**
- Modify: `src/companion.rs`（`prepare_import` 约 L401-406 处 + tests）

**Step 1: 写失败测试**（tests 模块内追加）

```rust
    #[test]
    fn test_import_registers_missing_motion_files() {
        run_with_temp_home(|home| {
            let src = home.join("火花");
            make_valid_model(&src, "火花.model3.json");
            add_unregistered_assets(&src);

            let (model, _) = import_from_dir(&src).unwrap();
            // 托管副本（非源目录）的清单被补注册。
            let json = read_manifest(Path::new(&model.model_file));
            let refs = json.get("FileReferences").unwrap();
            let extra = refs
                .get("Motions").and_then(|m| m.get("Extra")).and_then(|e| e.as_array()).unwrap();
            assert_eq!(extra.len(), 2);
            assert_eq!(refs.get("Expressions").and_then(|e| e.as_array()).unwrap().len(), 2);
            // 源目录清单保持原样（不污染用户文件）。
            let src_json = read_manifest(&src.join("火花.model3.json"));
            assert!(src_json.get("FileReferences").unwrap().get("Motions").is_none());
        });
    }
```

**Step 2: 运行确认失败**

```bash
cargo test --quiet test_import_registers 2>&1 | tail -3
```

Expected: FAIL（托管清单无 Extra 组）。

**Step 3: 实现**（`prepare_import` 中 `let (model_file_in_tmp, format) = ...` 之后、`let model_file_rel = ...` 之前插入）

```rust
    // 补注册托管副本中未登记的动作/表情文件（best-effort：失败仅告警，不阻塞导入；
    // 此时副本还在 tmp，源目录不受影响）。
    if let Err(e) = register_missing_motion_files(&model_file_in_tmp) {
        tracing::warn!("补注册动作/表情失败（不影响导入）: {e}");
    }
```

**Step 4: 运行测试通过**

```bash
cargo test --quiet companion:: 2>&1 | tail -3
```

Expected: 全部 PASS（含既有导入用例无回归）。

**Step 5: Commit**

```bash
git add src/companion.rs
git commit -m "feat(companion): 导入管线接入动作/表情补注册"
```

---

## Task 3: Rust — 存量模型一次性迁移

**Files:**
- Modify: `src/companion.rs`（`CompanionLibrary` 结构 + 新函数 + tests）
- Modify: `src-tauri/src/lib.rs`（后台触发，约 L3536 `migrate_legacy_in_background` 调用旁）

**Step 1: 写失败测试**（tests 模块内追加）

```rust
    #[test]
    fn test_register_motions_for_existing_migrates_and_is_idempotent() {
        run_with_temp_home(|home| {
            // 先用「无补注册能力」的方式造一个存量模型：手写 library.json + 托管目录。
            let src = home.join("存量");
            make_valid_model(&src, "old.model3.json");
            add_unregistered_assets(&src);
            let (model, _) = import_from_dir(&src).unwrap();
            // 手动还原清单为未注册状态，模拟「老版本导入、还没补注册」的存量。
            std::fs::write(
                Path::new(&model.model_file),
                r#"{"FileReferences":{"Moc":"model.moc3","Textures":["textures/texture_00.png"]}}"#,
            )
            .unwrap();

            let touched = register_motions_for_existing().unwrap();
            assert_eq!(touched, 1);
            let json = read_manifest(Path::new(&model.model_file));
            assert!(json.get("FileReferences").unwrap().get("Motions").is_some());

            // 标记已写入 → 二次调用直接跳过（即使再手动抹掉注册也不再处理）。
            let lib = load_library_fast().unwrap();
            assert!(lib
                .completed_migrations
                .iter()
                .any(|m| m == MOTION_REGISTRATION_MIGRATION));
            std::fs::write(
                Path::new(&model.model_file),
                r#"{"FileReferences":{"Moc":"model.moc3","Textures":["textures/texture_00.png"]}}"#,
            )
            .unwrap();
            assert_eq!(register_motions_for_existing().unwrap(), 0);
        });
    }

    #[test]
    fn test_library_json_without_migrations_field_still_loads() {
        run_with_temp_home(|_home| {
            let dir = get_companions_dir();
            std::fs::create_dir_all(&dir).unwrap();
            // 老版本 library.json（无 completed_migrations 字段）宽容加载。
            std::fs::write(
                dir.join(LIBRARY_FILE),
                r#"{"schema_version":1,"models":[],"active_model_id":null}"#,
            )
            .unwrap();
            let lib = load_library_fast().unwrap();
            assert!(lib.completed_migrations.is_empty());
        });
    }
```

**Step 2: 运行确认失败**

```bash
cargo test --quiet register_motions_for_existing 2>&1 | tail -3
```

Expected: 编译失败（`completed_migrations` 字段与函数不存在）。

**Step 3: 实现**

`CompanionLibrary`（`active_model_id` 字段后）加：

```rust
    /// 已完成的一次性迁移标记（如 "motion-registration-v1"）；缺失视为均未执行。
    /// 不 bump SCHEMA_VERSION：新字段对老文件是宽容默认，老代码读新文件忽略未知字段。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub completed_migrations: Vec<String>,
```

`Default for CompanionLibrary` 实现同步加 `completed_migrations: Vec::new(),`。

新函数（`migrate_legacy_if_empty` 之后）：

```rust
/// 存量伙伴一次性迁移：为库中所有伙伴补注册未登记的动作/表情文件。
///
/// 幂等闸门：`completed_migrations` 含本迁移标记即跳过。扫描/回写**不持锁**
/// （大目录 IO 不进临界区）；标记落库在短锁内重读后只追加标记字段，避免与
/// 并发导入互相覆盖。返回发生回写的模型数（0 = 无需处理或已迁移）。
pub fn register_motions_for_existing() -> Result<usize, String> {
    let models: Vec<CompanionModel> = {
        let _g = lock();
        let lib = load_library_inner()?;
        if lib
            .completed_migrations
            .iter()
            .any(|m| m == MOTION_REGISTRATION_MIGRATION)
        {
            return Ok(0);
        }
        lib.models
            .iter()
            .filter(|m| quick_valid(m))
            .cloned()
            .collect()
    };

    let mut touched = 0usize;
    for model in &models {
        match register_missing_motion_files(Path::new(&model.model_file)) {
            Ok(true) => touched += 1,
            Ok(false) => {}
            Err(e) => tracing::warn!("伙伴 {} 补注册动作/表情失败: {e}", model.id),
        }
    }

    {
        let _g = lock();
        let mut lib = load_library_inner()?;
        if !lib
            .completed_migrations
            .iter()
            .any(|m| m == MOTION_REGISTRATION_MIGRATION)
        {
            lib.completed_migrations
                .push(MOTION_REGISTRATION_MIGRATION.to_string());
            save_library_inner(&lib)?;
        }
    }
    Ok(touched)
}
```

`src-tauri/src/lib.rs`：`migrate_legacy_in_background` 函数（约 L2027）之后加：

```rust
/// 后台存量迁移：为已导入伙伴补注册未登记的动作/表情文件（幂等，不阻塞启动；
/// 失败不写标记，下次启动自动重试）。
fn register_motions_in_background() {
    tauri::async_runtime::spawn_blocking(|| match zapmomo::companion::register_motions_for_existing() {
        Ok(n) if n > 0 => tracing::info!("已为 {n} 个伙伴补注册动作/表情文件"),
        Ok(_) => {}
        Err(e) => tracing::warn!("补注册动作/表情迁移失败（下次启动重试）: {e}"),
    });
}
```

setup 内 `migrate_legacy_in_background(app.handle().clone());`（约 L3536）之后加一行：

```rust
            register_motions_in_background();
```

**Step 4: 运行测试通过**

```bash
cargo test --quiet companion:: 2>&1 | tail -3 && cargo check -p zapmomo-app --quiet 2>&1 | tail -3
```

Expected: 全部 PASS，tauri crate 编译通过。

**Step 5: fmt + clippy（两个 crate）**

```bash
cargo fmt && cargo clippy --quiet -- -D warnings 2>&1 | tail -2 && cargo clippy -p zapmomo-app --quiet -- -D warnings 2>&1 | tail -2
```

**Step 6: Commit**

```bash
git add src/companion.rs src-tauri/src/lib.rs
git commit -m "feat(companion): 启动时为存量伙伴补注册动作/表情（幂等迁移）"
```

---

## Task 4: 前端 — PreviewManager 能力枚举与播放接口

**Files:**
- Modify: `src-tauri/frontend/src/components/live2d/previewManager.ts`
- Test: `src-tauri/frontend/src/components/live2d/previewManager.test.ts`

**Step 1: 写失败测试**

先升级 mock：`vi.hoisted` 中 `fromMock` 生成的 model 增加 internalModel 结构：

```ts
    const fromMock = vi.fn(async (url: string) => {
      const model = {
        url,
        autoUpdate: false,
        destroy: vi.fn(),
        internalModel: {
          motionManager: {
            definitions:
              url === "asset://empty.model3.json"
                ? {}
                : { Extra: [{ File: "Motions/睡觉动画.motion3.json" }, { File: "chibang.motion3.json" }] },
            startMotion: vi.fn(async () => true),
          },
          expressionManager:
            url === "asset://empty.model3.json"
              ? undefined
              : {
                  definitions: [{ Name: "03 生气", File: "a.exp3.json" }, { Name: "07 星星眼", File: "b.exp3.json" }],
                  setExpression: vi.fn(async () => true),
                  resetExpression: vi.fn(),
                },
        },
      };
      modelInstances.push(model);
      return model;
    });
```

`makeCallbacks` 加 `onModelCatalog: vi.fn(),`。文件末尾追加 describe（模式照现有：`freshManager()` + `claim` 辅助；showModel 走 `await vi.waitFor` 等 promise 落地，同现有用例写法）：

```ts
describe("PreviewManager 动作/表情目录与播放", () => {
  it("attach 后回调 onModelCatalog（含展示名派生）；换模型/清屏后按语义更新", async () => {
    const manager = await freshManager();
    const { callbacks, handle } = claim(manager);
    handle.showModel("asset://a.model3.json", "default");
    await vi.waitFor(() => expect(callbacks.onModelCatalog).toHaveBeenCalled());
    expect(callbacks.onModelCatalog).toHaveBeenCalledWith({
      motionGroups: [
        {
          group: "Extra",
          motions: [
            { index: 0, name: "睡觉动画" },
            { index: 1, name: "chibang" },
          ],
        },
      ],
      expressions: [
        { index: 0, name: "03 生气" },
        { index: 1, name: "07 星星眼" },
      ],
    });
  });

  it("playMotion 以 FORCE 优先级转发；applyExpression/resetExpression 走 index；release 后 no-op", async () => {
    const manager = await freshManager();
    const { handle } = claim(manager);
    handle.showModel("asset://a.model3.json", "default");
    await vi.waitFor(() => expect(modelInstances.length).toBe(1));
    const model = modelInstances[0];

    expect(await handle.playMotion("Extra", 1)).toBe(true);
    expect(model.internalModel.motionManager.startMotion).toHaveBeenCalledWith("Extra", 1, 3);

    expect(await handle.applyExpression(0)).toBe(true);
    expect(model.internalModel.expressionManager!.setExpression).toHaveBeenCalledWith(0);

    handle.resetExpression();
    expect(model.internalModel.expressionManager!.resetExpression).toHaveBeenCalled();

    // 释放后（不再是当前占用者）播放安全 no-op。
    handle.release();
    expect(await handle.playMotion("Extra", 0)).toBe(false);
  });

  it("空目录模型回调空 catalog（两组空数组，非 null）", async () => {
    const manager = await freshManager();
    const { callbacks, handle } = claim(manager);
    handle.showModel("asset://empty.model3.json", "default");
    await vi.waitFor(() => expect(callbacks.onModelCatalog).toHaveBeenCalled());
    expect(callbacks.onModelCatalog).toHaveBeenLastCalledWith({
      motionGroups: [],
      expressions: [],
    });
  });
});
```

**Step 2: 运行确认失败**

```bash
cd src-tauri/frontend && pnpm vitest run previewManager.test.ts 2>&1 | tail -8
```

Expected: FAIL（`onModelCatalog` 未被调用 / `handle.playMotion` 不是函数）。

**Step 3: 实现**（`previewManager.ts`）

`PreviewSlotCallbacks` 加：

```ts
  /** 模型上舞台后回调其可播放的动作/表情目录（缓存命中也会触发；全量覆盖语义）。 */
  onModelCatalog?: (catalog: Live2dCatalog | null) => void;
```

`ClaimHandle` 加三个方法声明：

```ts
  /** 播放动作（FORCE 优先级可打断在播动作）；无当前模型或已释放时返回 false。 */
  playMotion(group: string, index: number): Promise<boolean>;
  /** 按下标应用表情（同名表情可能重复，index 无歧义）。 */
  applyExpression(index: number): Promise<boolean>;
  /** 重置表情回默认。 */
  resetExpression(): void;
```

文件顶部（`PreviewSlotCallbacks` 之前）加类型与枚举辅助：

```ts
/** 从已加载模型实例枚举的可播放能力（展示名从 File 派生）。 */
export interface Live2dCatalog {
  motionGroups: { group: string; motions: { index: number; name: string }[] }[];
  expressions: { index: number; name: string }[];
}

/** Cubism4 运行时动作/表情管理器的结构子集（基类类型不带具体定义，按结构断言）。 */
interface Live2dRuntime {
  motionManager: {
    definitions: Partial<Record<string, { File: string }[]>>;
    startMotion: (group: string, index: number, priority: number) => Promise<boolean>;
  };
  expressionManager?: {
    definitions: { Name: string }[];
    setExpression: (index: number) => Promise<boolean>;
    resetExpression: () => void;
  };
}

/** MotionPriority.FORCE = 3（枚举未从 cubism4 入口稳定导出，按库源码字面量传）。 */
const MOTION_PRIORITY_FORCE = 3;

/** 从模型实例读取断言后的运行时结构（buildCatalog 与播放共用）。 */
function runtimeOf(model: Live2DModel): Live2dRuntime {
  return model.internalModel as unknown as Live2dRuntime;
}

/** 动作展示名：File basename 去掉 .motion3.json 扩展。 */
function motionDisplayName(file: string): string {
  const name = file.split("/").pop() ?? file;
  return name.replace(/\.motion3\.json$/, "");
}

/** 枚举模型动作组与表情（纯函数，导出供测试）。 */
export function buildCatalog(model: Live2DModel): Live2dCatalog {
  const rt = runtimeOf(model);
  const motionGroups = Object.entries(rt.motionManager.definitions)
    .filter(([, defs]) => (defs?.length ?? 0) > 0)
    .map(([group, defs]) => ({
      group,
      motions: defs!.map((d, index) => ({ index, name: motionDisplayName(d.File) })),
    }));
  const expressions = (rt.expressionManager?.definitions ?? []).map((d, index) => ({
    index,
    name: d.Name,
  }));
  return { motionGroups, expressions };
}
```

`HandleImpl` 加方法转发：

```ts
  playMotion(group: string, index: number): Promise<boolean> {
    return this.manager.playMotion(this, group, index);
  }

  applyExpression(index: number): Promise<boolean> {
    return this.manager.applyExpression(this, index);
  }

  resetExpression(): void {
    this.manager.resetExpression(this);
  }
```

`PreviewManager` 加（放在 `showModel` 之后）：

```ts
  /** 仅当 handle 是当前占用者且有模型在播时返回模型，否则 null（stale-safe）。 */
  private modelFor(handle: HandleImpl): Live2DModel | null {
    return this.current?.id === handle.id ? this.shownModel : null;
  }

  /** 播放动作（FORCE 优先级打断 idle/在播动作）。 */
  playMotion(handle: HandleImpl, group: string, index: number): Promise<boolean> {
    const model = this.modelFor(handle);
    if (!model) return Promise.resolve(false);
    return runtimeOf(model).motionManager.startMotion(group, index, MOTION_PRIORITY_FORCE);
  }

  /** 按下标应用表情。 */
  applyExpression(handle: HandleImpl, index: number): Promise<boolean> {
    const model = this.modelFor(handle);
    if (!model) return Promise.resolve(false);
    return runtimeOf(model).expressionManager?.setExpression(index) ?? Promise.resolve(false);
  }

  /** 重置表情回默认。 */
  resetExpression(handle: HandleImpl): void {
    const model = this.modelFor(handle);
    model && runtimeOf(model).expressionManager?.resetExpression();
  }
```

`attach()` 末尾（`onModelReady` 回调之后）加：

```ts
    handle.callbacks.onModelCatalog?.(buildCatalog(entry.model));
```

`showModel` 的 `url === null` 分支（`this.detachShown()` 之后）加：

```ts
      handle.callbacks.onModelCatalog?.(null);
```

**Step 4: 运行测试通过**

```bash
cd src-tauri/frontend && pnpm vitest run previewManager.test.ts 2>&1 | tail -5
```

Expected: 新旧用例全 PASS。

**Step 5: Commit**

```bash
git add src-tauri/frontend/src/components/live2d/previewManager.ts src-tauri/frontend/src/components/live2d/previewManager.test.ts
git commit -m "feat(live2d): 共享舞台暴露动作/表情目录与播放接口"
```

---

## Task 5: 前端 — SharedLive2dStage 透传（ref + onModelCatalog）

**Files:**
- Modify: `src-tauri/frontend/src/components/live2d/SharedLive2dStage.tsx`
- Test: `src-tauri/frontend/src/components/live2d/SharedLive2dStage.test.tsx`

**Step 1: 写失败测试**

`handleStub` 补三个方法（vi.hoisted 内）：

```ts
  const handleStub = {
    id: 1,
    updateLayout: vi.fn(),
    showModel: vi.fn(),
    release: vi.fn(),
    playMotion: vi.fn(async () => true),
    applyExpression: vi.fn(async () => true),
    resetExpression: vi.fn(),
  };
```

`beforeEach` 补三个 `mockClear()`。describe 内追加用例：

```tsx
  it("透传 onModelCatalog 回调，并通过 ref 暴露播放/重置方法", async () => {
    const ref = { current: null as unknown as SharedLive2dStageHandle };
    const onModelCatalog = vi.fn();
    render(
      <SharedLive2dStage {...baseProps()} ref={ref} onModelCatalog={onModelCatalog} />,
    );

    const opts = claimMock.mock.calls[0][0];
    expect(opts.callbacks).toHaveProperty("onModelCatalog");

    expect(await ref.current!.playMotion("Extra", 0)).toBe(true);
    expect(handleStub.playMotion).toHaveBeenCalledWith("Extra", 0);
    expect(await ref.current!.applyExpression(1)).toBe(true);
    expect(handleStub.applyExpression).toHaveBeenCalledWith(1);
    ref.current!.resetExpression();
    expect(handleStub.resetExpression).toHaveBeenCalledTimes(1);

    // 卸载后 ref 方法安全 no-op（不抛错、返回 false）。
    // （unmount 后 handleRef.current 为 null，转发层兜底 false。）
  });
```

**Step 2: 运行确认失败**

```bash
cd src-tauri/frontend && pnpm vitest run SharedLive2dStage 2>&1 | tail -5
```

Expected: FAIL（ref 上无方法 / callbacks 无 onModelCatalog）。

**Step 3: 实现**（`SharedLive2dStage.tsx`）

import 区加 `useImperativeHandle`、`type Ref`，并从 `./previewManager` 导入 `Live2dCatalog`。组件 props 加两个可选成员，并导出 handle 类型：

```tsx
/** SharedLive2dStage 的命令式句柄：动作/表情播放（卸载后调用安全 no-op）。 */
export type SharedLive2dStageHandle = {
  playMotion: (group: string, index: number) => Promise<boolean>;
  applyExpression: (index: number) => Promise<boolean>;
  resetExpression: () => void;
};

interface SharedLive2dStageProps {
  // …既有字段不变…
  /** 模型动作/表情目录就绪回调（缓存命中也会触发，全量覆盖语义）。 */
  onModelCatalog?: (catalog: Live2dCatalog | null) => void;
  /** 命令式句柄（React 19 ref as prop）：播放动作/表情与重置。 */
  ref?: Ref<SharedLive2dStageHandle>;
}
```

组件签名解构加 `onModelCatalog, ref`；`callbacksRef` 初始化加字段并每渲染更新：

```tsx
  const callbacksRef = useRef<PreviewSlotCallbacks>({});
  callbacksRef.current.onError = onError;
  callbacksRef.current.onModelMetrics = onModelMetrics;
  callbacksRef.current.onModelReady = onModelReady;
  callbacksRef.current.onModelCatalog = onModelCatalog;
```

挂载 effect 之后加：

```tsx
  // 命令式句柄：转发到当前 claim 的 handle；释放后（handleRef.current 为 null）安全 no-op。
  useImperativeHandle(
    ref,
    () => ({
      playMotion: (group, index) =>
        handleRef.current?.playMotion(group, index) ?? Promise.resolve(false),
      applyExpression: (index) =>
        handleRef.current?.applyExpression(index) ?? Promise.resolve(false),
      resetExpression: () => handleRef.current?.resetExpression(),
    }),
    [],
  );
```

**Step 4: 运行测试通过**

```bash
cd src-tauri/frontend && pnpm vitest run SharedLive2dStage 2>&1 | tail -5
```

Expected: 全 PASS。

**Step 5: Commit**

```bash
git add src-tauri/frontend/src/components/live2d/SharedLive2dStage.tsx src-tauri/frontend/src/components/live2d/SharedLive2dStage.test.tsx
git commit -m "feat(live2d): SharedLive2dStage 透传目录回调与播放句柄"
```

---

## Task 6: 前端 — 伙伴页动作/表情面板

**Files:**
- Modify: `src-tauri/frontend/src/pages/CompanionPage.tsx`（新子组件 `MotionCatalogPanel` + 接线）
- Test: `src-tauri/frontend/src/pages/CompanionPage.test.tsx`

**Step 1: 升级测试 mock 并写失败用例**

`CompanionPage.test.tsx` 顶部 mock 替换（保留原 div，增加目录注入与句柄注入）：

```tsx
// SharedLive2dStage 依赖 pixi / WebGL，jsdom 无法运行；mock 成可注入目录与句柄的替身。
const { stageHandleMock } = vi.hoisted(() => ({
  stageHandleMock: {
    playMotion: vi.fn(async () => true),
    applyExpression: vi.fn(async () => true),
    resetExpression: vi.fn(),
  },
}));
/** 控制替身组件挂载时注入的目录（每个用例自行设置，afterEach 复位）。 */
let stageCatalog: import("@/components/live2d/previewManager").Live2dCatalog | null;

vi.mock("@/components/live2d/SharedLive2dStage", async () => {
  const { useEffect } = await import("react");
  return {
    SharedLive2dStage: ({
      onModelCatalog,
      ref,
    }: {
      onModelCatalog?: (c: import("@/components/live2d/previewManager").Live2dCatalog | null) => void;
      ref?: { current: unknown };
    }) => {
      useEffect(() => {
        onModelCatalog?.(stageCatalog);
        if (ref) ref.current = stageHandleMock;
      }, [onModelCatalog, ref]);
      return <div data-testid="live2d-stage" />;
    },
  };
});
```

`beforeEach` 加 `stageHandleMock.playMotion.mockClear()` 等三个清理 + `stageCatalog = null;`。

新用例：

```tsx
  it("展示动作与表情目录：点击动作播放、点击表情应用、重置恢复", async () => {
    library = { models: [MODEL_A], active_model_id: MODEL_A.id };
    stageCatalog = {
      motionGroups: [{ group: "Extra", motions: [{ index: 0, name: "睡觉动画" }, { index: 1, name: "循环动画" }] }],
      expressions: [{ index: 0, name: "03 生气" }, { index: 1, name: "07 星星眼" }],
    };
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("tab", { name: "动作" }));
    await user.click(screen.getByRole("button", { name: "播放动作 睡觉动画" }));
    expect(stageHandleMock.playMotion).toHaveBeenCalledWith("Extra", 0);

    await user.click(screen.getByRole("tab", { name: "表情" }));
    await user.click(screen.getByRole("button", { name: "应用表情 07 星星眼" }));
    expect(stageHandleMock.applyExpression).toHaveBeenCalledWith(1);
    await user.click(screen.getByRole("button", { name: "重置表情" }));
    expect(stageHandleMock.resetExpression).toHaveBeenCalledTimes(1);
  });

  it("模型没有动作与表情时显示空态；目录未就绪时不渲染面板", async () => {
    library = { models: [MODEL_A], active_model_id: MODEL_A.id };
    stageCatalog = { motionGroups: [], expressions: [] };
    renderPage();
    expect(await screen.findByText("此模型未提供动作或表情")).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "动作" })).not.toBeInTheDocument();
  });
```

**Step 2: 运行确认失败**

```bash
cd src-tauri/frontend && pnpm vitest run CompanionPage 2>&1 | tail -5
```

Expected: 两个新用例 FAIL（找不到 tab）。

**Step 3: 实现**（`CompanionPage.tsx`）

import 区：`import { SharedLive2dStage, type SharedLive2dStageHandle } from "@/components/live2d/SharedLive2dStage";`、`import type { Live2dCatalog } from "@/components/live2d/previewManager";`、`lucide-react` 图标按需（沿用现有）。

新子组件（放在 `CompanionListItem` 之后，样式对齐现有 outline 按钮）：

```tsx
/** 动作/表情预览面板：手写下划线 Tab（对齐 ModelDetailPane 先例，不引 Tabs 依赖）。 */
function MotionCatalogPanel({
  catalog,
  onPlayMotion,
  onApplyExpression,
  onResetExpression,
}: {
  catalog: Live2dCatalog;
  onPlayMotion: (group: string, index: number) => void;
  onApplyExpression: (index: number) => void;
  onResetExpression: () => void;
}) {
  const [tab, setTab] = useState<"motions" | "expressions">("motions");
  const [playingKey, setPlayingKey] = useState<string | null>(null);
  const [appliedExpression, setAppliedExpression] = useState<number | null>(null);

  const hasMotions = catalog.motionGroups.length > 0;
  const hasExpressions = catalog.expressions.length > 0;
  const activeTab = tab === "motions" && !hasMotions ? "expressions" : tab;

  const handlePlay = (group: string, index: number) => {
    setPlayingKey(`${group}/${index}`);
    // 播放结束（或失败）后恢复按钮；首次播放含懒加载延迟。
    void Promise.resolve(onPlayMotion(group, index)).finally(() => setPlayingKey(null));
  };

  const empty = !hasMotions && !hasExpressions;
  return (
    <div className="mt-3 shrink-0 border-t border-panel-border pt-3" data-testid="motion-catalog">
      {empty ? (
        <p className="text-xs text-muted-foreground">此模型未提供动作或表情</p>
      ) : (
        <>
          {hasMotions && hasExpressions && (
            <div role="tablist" aria-label="预览类型" className="mb-2 flex gap-4">
              {(["motions", "expressions"] as const).map((t) => (
                <button
                  key={t}
                  type="button"
                  role="tab"
                  aria-selected={activeTab === t}
                  onClick={() => setTab(t)}
                  className={cn(
                    "border-b-2 pb-1 text-sm transition-colors",
                    activeTab === t
                      ? "border-blue-500 text-text-primary"
                      : "border-transparent text-muted-foreground hover:text-text-primary",
                  )}
                >
                  {t === "motions" ? "动作" : "表情"}
                </button>
              ))}
            </div>
          )}
          <div className="flex max-h-40 flex-wrap content-start gap-2 overflow-y-auto">
            {activeTab === "motions"
              ? catalog.motionGroups.map(({ group, motions }) => (
                  <div key={group} className="w-full">
                    {catalog.motionGroups.length > 1 && (
                      <p className="mb-1 text-xs font-medium text-muted-foreground">{group}</p>
                    )}
                    <div className="flex flex-wrap gap-2">
                      {motions.map((m) => {
                        const key = `${group}/${m.index}`;
                        return (
                          <Button
                            key={key}
                            variant="outline"
                            size="sm"
                            aria-label={`播放动作 ${m.name}`}
                            disabled={playingKey !== null}
                            onClick={() => handlePlay(group, m.index)}
                          >
                            {playingKey === key ? "播放中…" : m.name}
                          </Button>
                        );
                      })}
                    </div>
                  </div>
                ))
              : catalog.expressions.map((e) => (
                  <Button
                    key={e.index}
                    variant={appliedExpression === e.index ? "default" : "outline"}
                    size="sm"
                    aria-label={`应用表情 ${e.name}`}
                    onClick={() => {
                      onApplyExpression(e.index);
                      setAppliedExpression(e.index);
                    }}
                  >
                    {e.name}
                  </Button>
                ))}
            {activeTab === "expressions" && appliedExpression != null && (
              <Button
                variant="ghost"
                size="sm"
                aria-label="重置表情"
                onClick={() => {
                  onResetExpression();
                  setAppliedExpression(null);
                }}
              >
                重置表情
              </Button>
            )}
          </div>
        </>
      )}
    </div>
  );
}
```

`CompanionPage` 内接线（state 区加）：

```tsx
  const stageHandleRef = useRef<SharedLive2dStageHandle>(null);
  const [catalog, setCatalog] = useState<Live2dCatalog | null>(null);
  // 模型切换瞬间清空旧目录（onModelCatalog 全量覆盖，无需额外去重）。
  useEffect(() => {
    setCatalog(null);
  }, [selectedId]);
```

`SharedLive2dStage` 调用处加 props：

```tsx
                <SharedLive2dStage
                  modelUrl={previewUrl}
                  width={previewSize.width}
                  height={previewSize.height}
                  onError={handleStageError}
                  onModelReady={handleModelReady}
                  onModelCatalog={setCatalog}
                  ref={stageHandleRef}
                  className="h-full w-full"
                />
```

预览区 div（`<div ref={previewRef} …>`）之后、stageError Alert 之前渲染面板：

```tsx
            {showStage && catalog && (
              <MotionCatalogPanel
                catalog={catalog}
                onPlayMotion={(group, index) => stageHandleRef.current?.playMotion(group, index)}
                onApplyExpression={(index) => void stageHandleRef.current?.applyExpression(index)}
                onResetExpression={() => stageHandleRef.current?.resetExpression()}
              />
            )}
```

**Step 4: 运行测试通过**

```bash
cd src-tauri/frontend && pnpm vitest run CompanionPage 2>&1 | tail -5
```

Expected: 全 PASS（含既有用例无回归）。

**Step 5: Commit**

```bash
git add src-tauri/frontend/src/pages/CompanionPage.tsx src-tauri/frontend/src/pages/CompanionPage.test.tsx
git commit -m "feat(companion): 伙伴页动作/表情预览面板"
```

---

## Task 7: 全量验收

**Step 1: Rust 全量**

```bash
cargo fmt --check && cargo clippy --quiet -- -D warnings && cargo test --quiet 2>&1 | tail -5
```

Expected: 全绿。

**Step 2: 前端全量**

```bash
cd src-tauri/frontend && pnpm tsc -b && pnpm vitest run 2>&1 | tail -5
```

Expected: 类型检查零错误、全部测试 PASS。

**Step 3: 人工验收清单（`pnpm tauri dev`，需要真机）**

1. 移除并重新导入「火花」：伙伴页出现 2 动作 + 10 表情，点击播放（首次有短暂加载）；
2. 存量「火花」不重导入：启动后目录同样出现（迁移）；二次启动日志无重复回写；
3. 「cat」模型显示「此模型未提供动作或表情」；
4. 桌宠窗口行为不变：不自动播放动作、呼吸/眨眼照旧、点击无反应；
5. 表情应用/重置生效；概览页 ⇄ 伙伴页切换目录正常恢复；
6. `~/.zapmomo/companions/<id>/*.model3.json` 出现 `"Extra"` 组与 Expressions，Moc/Textures 原样。

**Step 4: 最终提交（如 fmt 产生变更）并汇报**

---

## 回归风险备忘

- 任何 `cargo test` 出现 env 竞争报错 → 用 `cargo test -- --test-threads=1` 复核（CLAUDE.md 已知事项）。
- 前端根目录 `tsc --noEmit` 空通过不可信，必须 `tsc -b`（项目记忆）。
- `previewManager.test.ts` 的 mock 必须用普通 `function` 构造（Vitest 4 限制，文件头已有注释）。
- pixi-live2d-display 依赖不可升级（pixi 6 锁定，项目记忆）。
