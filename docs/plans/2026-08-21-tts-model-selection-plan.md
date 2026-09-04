# TTS「选择模型」弹窗 Implementation Plan（第一期：铺管线）

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 「模型与能力」页为 TTS 提供与 KWS/ASR/LLM 同款的「选择模型」弹窗（下载 / 设为当前 / 卸载 / 进度），并注入 TTS 运行状态。

**Architecture:** 镜像 PR #134（ASR 选择模型）模式：后端 `RuntimeActuals` 增加 tts 字段注入合成状态、`set_selected_model` TTS 分支切换时重置文件级覆盖；前端新增 `useTtsModelSwitch` + `TtsModelDialog` + `TtsModelSwitchMenu` 三件套并接入 `ModelSummary` / `TtsPage` / `TtsBasicConfig`，同时移除前端 legacy 一键下载通道（后端 `download_tts_model` command 保留不动）。

**Tech Stack:** Rust（根 crate `zapmomo` + `src-tauri/`）、React 19 + TypeScript + Vitest + Testing Library。

设计文档：`docs/plans/2026-08-21-tts-model-selection-design.md`

**环境注意（worktree）：**
- 所有 cargo 命令在仓库根执行；若 sherpa-onnx-sys 构建因磁盘/下载失败，用 `CARGO_TARGET_DIR=/Users/nemo/Projects/shenjingnan/zapmomo/target cargo …` 共享主仓库 target。
- Rust 测试按项目惯例单线程：`cargo test -- --test-threads=1`。
- 前端命令均在 `src-tauri/frontend/` 下：`pnpm test:run <file>`（vitest run）、`pnpm exec tsc -b`（类型检查用 `tsc -b`，不要用根目录 `tsc --noEmit`）。

---

## Task 1: Rust — `set_selected_model` TTS 分支重置文件级覆盖

**Files:**
- Modify: `src/model_library/mod.rs:386-388`（`set_selected_model` 的 `ModelType::Tts` 分支）
- Test: `src/model_library/mod.rs`（`tests` 模块，加在 `test_set_selected_asr_resets_file_overrides` 之后，约 :1415）

**Step 1: 写失败测试**

在 `src/model_library/mod.rs` 的 `tests` 模块末尾（`test_set_selected_asr_resets_file_overrides` 之后）追加：

```rust
    #[test]
    fn test_set_selected_tts_resets_file_overrides() {
        run_with_temp_home(|home| {
            // 预写旧模型的文件级覆盖（模拟手工改过的配置）
            update_settings(|cfg| {
                let tts = cfg.tts.get_or_insert_with(Default::default);
                tts.model_dir = Some("old-model".to_string());
                tts.encoder = Some("old-encoder.onnx".to_string());
                tts.decoder = Some("old-decoder.onnx".to_string());
                tts.vocoder = Some("old-vocoder.onnx".to_string());
                tts.tokens = Some("old-tokens.txt".to_string());
                tts.lexicon = Some("old-lexicon.txt".to_string());
                tts.data_dir = Some("old-espeak-ng-data".to_string());
                tts.reference_wav = Some("old-ref.wav".to_string());
                tts.reference_text = Some("旧参考文本".to_string());
                tts.enabled = Some(true);
                tts.voice = Some("leijun-1".to_string());
            })
            .unwrap();

            // 切换到新模型目录
            let new_dir = home.join("models/zipvoice");
            set_selected_model(ModelType::Tts, &new_dir).unwrap();

            let cfg = settings::load_settings().unwrap().unwrap();
            let tts = cfg.tts.as_ref().expect("tts 段应存在");
            assert_eq!(
                tts.model_dir,
                Some(new_dir.to_string_lossy().to_string()),
                "model_dir 应更新"
            );
            // 文件级覆盖全部重置：交回 resolve 按目录探测
            assert_eq!(tts.encoder, None);
            assert_eq!(tts.decoder, None);
            assert_eq!(tts.vocoder, None);
            assert_eq!(tts.tokens, None);
            assert_eq!(tts.lexicon, None);
            assert_eq!(tts.data_dir, None);
            // reference_wav/text 是旧模型目录内的参考音频，一并重置回默认音色
            assert_eq!(tts.reference_wav, None);
            assert_eq!(tts.reference_text, None);
            // enabled / 音色偏好 / 参数不受切换影响
            assert_eq!(tts.enabled, Some(true));
            assert_eq!(tts.voice, Some("leijun-1".to_string()));
        });
    }
```

**Step 2: 跑测试确认失败**

```bash
cargo test test_set_selected_tts_resets_file_overrides -- --test-threads=1
```
预期：FAIL（`tts.encoder` 等仍为 `Some("old-…")`）。

**Step 3: 实现**

`src/model_library/mod.rs:386-388` 的 Tts 分支替换为：

```rust
        ModelType::Tts => {
            let tts = cfg.tts.get_or_insert_with(Default::default);
            tts.model_dir = Some(path_str);
            // 切换模型目录时重置文件级覆盖：旧模型的手写覆盖（encoder/vocoder 等）
            // 会污染新模型的文件探测，交回 resolve 自动探测（与 KWS/ASR 分支同款取舍）。
            // reference_wav/text 指向旧模型目录内的参考音频，一并重置回默认音色；
            // enabled / voice / num_steps / speed 等用户偏好不重置。
            tts.encoder = None;
            tts.decoder = None;
            tts.vocoder = None;
            tts.tokens = None;
            tts.lexicon = None;
            tts.data_dir = None;
            tts.reference_wav = None;
            tts.reference_text = None;
        }
```

**Step 4: 跑测试确认通过**

```bash
cargo test test_set_selected_tts_resets_file_overrides -- --test-threads=1
cargo fmt && cargo clippy -- -D warnings
```
预期：PASS / 无警告。

**Step 5: Commit**

```bash
git add src/model_library/mod.rs
git commit -m "feat(tts): 切换 TTS 模型时重置文件级覆盖配置"
```

---

## Task 2: Rust — `RuntimeActuals` 注入 TTS 运行状态

**Files:**
- Modify: `src/model_library/mod.rs:181-188`（`RuntimeActuals` 结构）
- Modify: `src/model_library/mod.rs:456`（`enrich_runtime_status` 的 Tts 分支）
- Modify: `src-tauri/src/lib.rs:3376-3403`（`list_model_library` 注入）
- Test: `src/model_library/mod.rs`（tests 模块）

**Step 1: 写失败测试**

在 tests 模块追加（含一个最小 `LibraryModel` 构造 helper）：

```rust
    /// 测试用最小 TTS LibraryModel（enrich 只读 current / model_type / local_path）。
    fn tts_library_model(current: bool) -> LibraryModel {
        LibraryModel {
            id: "tts-zipvoice-distill-int8".to_string(),
            name: "sherpa-onnx-zipvoice-distill-int8-zh-en-emilia".to_string(),
            display_name: "ZipVoice TTS zh-en".to_string(),
            model_type: ModelType::Tts,
            runtime: "sherpa-onnx".to_string(),
            format: "ONNX".to_string(),
            description: String::new(),
            languages: vec![],
            tags: vec![],
            parameter_count: None,
            quantization: None,
            version: "distill-int8".to_string(),
            size_bytes: None,
            homepage: None,
            downloadable: true,
            source: ModelSource::Registry,
            ownership: StorageOwnership::Managed,
            install_state: InstallState::Installed,
            current,
            runtime_status: RuntimeStatus::Inactive,
            local_path: Some("/models/zipvoice".to_string()),
            installed_at: None,
            install_id: None,
            repo_id: None,
            compatibility: None,
        }
    }

    #[test]
    fn test_enrich_runtime_status_tts() {
        let dir = Path::new("/models/zipvoice");
        // 合成中：当前模型 Active、非当前恒 Inactive
        let mut models = vec![tts_library_model(true), tts_library_model(false)];
        let actuals = RuntimeActuals {
            kws: None,
            asr: None,
            tts: Some(dir),
            tts_active: true,
            llm: None,
            llm_switching: false,
            llm_switch_target: None,
            llm_load_error_path: None,
        };
        enrich_runtime_status(&mut models, &actuals);
        assert_eq!(models[0].runtime_status, RuntimeStatus::Active);
        assert_eq!(models[1].runtime_status, RuntimeStatus::Inactive);

        // 空闲（无合成线程）：当前模型 Inactive
        let mut models = vec![tts_library_model(true)];
        let actuals = RuntimeActuals {
            tts_active: false,
            ..actuals
        };
        enrich_runtime_status(&mut models, &actuals);
        assert_eq!(models[0].runtime_status, RuntimeStatus::Inactive);
    }
```

**Step 2: 跑测试确认编译失败**

```bash
cargo test test_enrich_runtime_status_tts -- --test-threads=1
```
预期：COMPILE FAIL（`RuntimeActuals` 无 `tts` / `tts_active` 字段）。

**Step 3: 实现**

1. `RuntimeActuals`（mod.rs:181）增加两个字段（放 `asr` 之后、`llm` 之前，语义按能力排序）：

```rust
pub struct RuntimeActuals<'a> {
    pub kws: Option<&'a Path>,
    pub asr: Option<&'a Path>,
    /// TTS 无常驻引擎：actual = 当前 selection（与 current 判定同源）
    pub tts: Option<&'a Path>,
    /// 是否有合成线程在跑（在飞合成用旧配置完成，下次合成读新配置）
    pub tts_active: bool,
    pub llm: Option<&'a Path>,
    pub llm_switching: bool,
    pub llm_switch_target: Option<&'a Path>,
    pub llm_load_error_path: Option<&'a Path>,
}
```

2. `enrich_runtime_status`（:456）：

```rust
            ModelType::Tts => (a.tts, a.tts_active),
```

3. `src-tauri/src/lib.rs` `list_model_library`（:3376）：签名增加 `tts: State<'_, TtsSynthesizeState>` 参数（放在 `llm` 之后），并在 `actuals` 构造处（:3393）补充：

```rust
    // TTS 无常驻引擎：actual = 当前 selection（与 current 判定同源，写配置即切换），
    // active = 是否有合成线程在跑。
    let tts_actual = model_library::selection_path(LibModelType::Tts);
    let actuals = model_library::RuntimeActuals {
        kws: kws_actual.as_deref(),
        asr: asr_actual.as_deref(),
        tts: tts_actual.as_deref(),
        tts_active: tts.is_synthesizing(),
        llm: llm_actual.as_deref(),
        llm_switching: llm.is_switching(),
        llm_switch_target: llm_target.as_deref(),
        llm_load_error_path: llm_error_path.as_deref(),
    };
```

**Step 4: 跑测试确认通过**

```bash
cargo test test_enrich_runtime_status_tts -- --test-threads=1
cargo check -p zapmomo-app && cargo fmt && cargo clippy -- -D warnings
```
预期：PASS / 无警告。

**Step 5: Commit**

```bash
git add src/model_library/mod.rs src-tauri/src/lib.rs
git commit -m "feat(tts): 模型库运行状态注入 TTS 合成状态"
```

---

## Task 3: 前端 — `useTtsModelSwitch` hook + `TTS_PRESETS`

**Files:**
- Create: `src-tauri/frontend/src/hooks/useTtsModelSwitch.ts`

**Step 1: 创建 hook**（无独立 hook 测试，与 ASR PR #134 同策略：交互经 Task 5/6/7 的组件测试覆盖）

`src-tauri/frontend/src/hooks/useTtsModelSwitch.ts` 完整内容：

```ts
import { useCallback, useEffect, useRef, useState } from "react";
import { useToast } from "@/components/ui/toast";
import { api, onModelLibraryDownloadProgress } from "@/lib/tauri";
import { useRuntime } from "@/providers/RuntimeContext";
import type { LibraryModel, ModelLibraryProgress, SetCurrentResult } from "@/types/modelLibrary";

/** TTS 切换弹窗的内置预设（id = models/model_registry.json 的 registry id）。 */
export const TTS_PRESETS = [
  {
    id: "tts-zipvoice-distill-int8",
    name: "ZipVoice TTS zh-en",
    tagline: "零样本声音克隆 · 中英双语 · 含声码器",
    sizeBytes: 163_320_194,
  },
] as const;

export interface TtsModelSwitchState {
  /** `list_model_library` 快照（含安装 / current 状态）；null = 尚未加载 */
  models: LibraryModel[] | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  /** 下载 registry 模型（model-library-download-progress 进度） */
  download: (id: string) => Promise<void>;
  downloadingId: string | null;
  progress: ModelLibraryProgress | null;
  /** 设为当前模型；TTS 每次合成现场建引擎，写完配置即生效（下次合成用新模型） */
  setCurrent: (id: string) => Promise<void>;
  /** 卸载（managed 删文件；当前/下载中模型后端会拒绝） */
  remove: (id: string) => Promise<void>;
}

/**
 * TTS 模型切换状态：从模型库列表过滤 TTS 条目，提供下载 / 设为当前 / 卸载。
 * 数据用 `list_model_library`（与模型库页同一后端真相源，含 install_state + current）。
 * 与 ASR 版差异：TTS 无监听概念，切换不需要重启任何 runtime。
 */
export function useTtsModelSwitch(): TtsModelSwitchState {
  const runtime = useRuntime();
  const toast = useToast();
  const [models, setModels] = useState<LibraryModel[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [progress, setProgress] = useState<ModelLibraryProgress | null>(null);
  /** 下载终态（done/cancelled/failed）：await 返回时事件可能尚未到达，用 ref 透传 */
  const terminalStage = useRef<string | null>(null);

  // setCurrent 的 await 期间 runtime 可能变化（刷新配置需读最新 tts 切片）
  const runtimeRef = useRef(runtime);
  runtimeRef.current = runtime;

  const refresh = useCallback(async () => {
    try {
      setModels(await api.listModelLibrary());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const unlisten = onModelLibraryDownloadProgress((p) => {
      setProgress(p);
      if (p.stage === "done" || p.stage === "cancelled" || p.stage === "failed") {
        terminalStage.current = p.stage;
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const download = useCallback(
    async (id: string) => {
      setDownloadingId(id);
      setProgress(null);
      terminalStage.current = null;
      try {
        await api.downloadLibraryModel({ id });
        const stage = terminalStage.current;
        if (stage === "cancelled") {
          toast.warning("已取消下载");
        } else {
          const name = TTS_PRESETS.find((p) => p.id === id)?.name ?? id;
          toast.success(`✓ ${name} 下载完成`);
        }
      } catch (e) {
        toast.error(`模型下载失败：${String(e)}`);
      } finally {
        setDownloadingId(null);
        setProgress(null);
        terminalStage.current = null;
        await refresh();
      }
    },
    [toast, refresh],
  );

  const setCurrent = useCallback(
    async (id: string) => {
      let res: SetCurrentResult;
      try {
        res = await api.setCurrentModel({ id });
      } catch (e) {
        toast.error(String(e));
        return;
      }
      // 刷新 TTS 配置（当前模型名/就绪状态）与模型库列表；后端只写配置即生效
      await Promise.allSettled([runtimeRef.current.tts.refreshConfig(), refresh()]);
      toast.success(res.message);
    },
    [toast, refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      try {
        await api.deleteModel({ id });
        toast.success("✓ 模型已卸载");
      } catch (e) {
        toast.error(String(e));
        return;
      }
      await Promise.allSettled([runtimeRef.current.tts.refreshConfig(), refresh()]);
    },
    [toast, refresh],
  );

  return { models, loading, error, refresh, download, downloadingId, progress, setCurrent, remove };
}
```

**Step 2: 类型检查**

```bash
cd src-tauri/frontend && pnpm exec tsc -b
```
预期：无错误（hook 尚无消费者，仅编译）。

**Step 3: Commit**

```bash
git add src-tauri/frontend/src/hooks/useTtsModelSwitch.ts
git commit -m "feat(tts): 新增 TTS 模型切换 hook（useTtsModelSwitch）"
```

---

## Task 4: 前端 — `TtsModelDialog` 弹窗组件

**Files:**
- Create: `src-tauri/frontend/src/components/tts/TtsModelDialog.tsx`

**Step 1: 创建组件**（镜像 `AsrModelDialog.tsx`；差异：无 `switchingId`（TTS 切换是瞬时写配置）、无语音会话警示 Alert（静默生效决策））

`src-tauri/frontend/src/components/tts/TtsModelDialog.tsx` 完整内容：

```tsx
import { CircleAlert, Download, Trash2 } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { LibraryDialog } from "@/components/library/LibraryDialog";
import { ModelConfirmDialog } from "@/components/library/LibraryDialogs";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { TTS_PRESETS, useTtsModelSwitch } from "@/hooks/useTtsModelSwitch";
import { useSmoothProgress } from "@/hooks/useSmoothProgress";
import { formatBytes } from "@/lib/catalog/quantization";
import type { LibraryModel } from "@/types/modelLibrary";

interface TtsModelDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * 选择合成模型弹窗（与 KWS/ASR/LLM 选择模型弹窗同款交互）：
 * 内置预设（未安装→下载；已安装→设为当前 / 卸载；当前→标记）。
 * TTS 每次合成现场建引擎：切换立即生效（下次合成使用新模型）；
 * 语音会话运行中也静默生效（下次会话用新模型），无需重启提示。
 * 卸载确认框嵌套在此弹窗内。
 */
export function TtsModelDialog({ open, onClose }: TtsModelDialogProps) {
  const switcher = useTtsModelSwitch();
  const [confirmModel, setConfirmModel] = useState<LibraryModel | null>(null);
  const { downloadingId, progress } = switcher;

  // verifying/done 等阶段后端 overallPercent=-1，非 downloading 一律按 100
  const targetPercent =
    progress?.stage === "downloading" ? Math.max(0, Math.min(100, progress.overallPercent)) : 100;
  // 平滑插值：消除高频进度事件造成的进度条抖动
  const percent = useSmoothProgress(targetPercent);

  return (
    <LibraryDialog open={open} onClose={onClose} title="选择合成模型" width="lg">
      <p className="text-xs text-text-muted">
        内置语音合成模型：下载后即可设为当前；切换立即生效（下次合成使用新模型）。
      </p>

      <div className="space-y-2">
        {TTS_PRESETS.map((p) => {
          // 仅「完整已安装」视为已安装（list_model_library 对未安装 registry 模型也返回记录）
          const installed =
            (switcher.models ?? []).find(
              (m) => (m.id === p.id || m.repoId === p.id) && m.installState === "installed",
            ) ?? null;
          const busy = downloadingId === p.id;
          return (
            <div
              key={p.id}
              className="flex items-center justify-between gap-3 rounded-lg border border-panel-border px-3 py-2.5"
            >
              <div className="min-w-0">
                <p className="text-sm font-medium text-text-primary">{p.name}</p>
                <p className="mt-0.5 text-xs text-text-muted">
                  {`${formatBytes(p.sizeBytes)} · ${p.tagline}`}
                </p>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {installed ? (
                  installed.current ? (
                    <span className="inline-flex items-center gap-1.5 text-xs text-emerald-600">
                      <span className="h-1.5 w-1.5 rounded-full bg-current" />
                      当前模型
                    </span>
                  ) : (
                    <>
                      <Button size="sm" onClick={() => void switcher.setCurrent(installed.id)}>
                        设为当前
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        className="shadow-none text-destructive hover:text-destructive"
                        onClick={() => setConfirmModel(installed)}
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                        卸载
                      </Button>
                    </>
                  )
                ) : (
                  <Button
                    size="sm"
                    onClick={() => void switcher.download(p.id)}
                    disabled={downloadingId !== null}
                    aria-label={`下载${p.name}`}
                  >
                    <Download className="h-4 w-4" />
                    {busy ? "下载中…" : "下载"}
                  </Button>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {progress && (
        <div className="space-y-1">
          <Progress value={percent} />
          <p className="text-xs text-text-muted">{progress.message}</p>
        </div>
      )}

      {switcher.error && (
        <Alert variant="destructive">
          <CircleAlert className="h-4 w-4" />
          <AlertDescription className="whitespace-pre-wrap">
            读取模型列表失败：{switcher.error}
          </AlertDescription>
        </Alert>
      )}

      <div className="flex flex-wrap items-center gap-2 pt-1">
        <Link
          to="/models/library"
          onClick={onClose}
          className="text-xs text-text-secondary transition-colors hover:text-text-primary"
        >
          更多模型 → 模型库
        </Link>
      </div>

      <ModelConfirmDialog
        open={confirmModel !== null}
        model={confirmModel}
        onClose={() => setConfirmModel(null)}
        onConfirm={(m) => {
          setConfirmModel(null);
          void switcher.remove(m.id);
        }}
      />
    </LibraryDialog>
  );
}
```

**Step 2: 类型检查**

```bash
cd src-tauri/frontend && pnpm exec tsc -b
```
预期：无错误。

**Step 3: Commit**

```bash
git add src-tauri/frontend/src/components/tts/TtsModelDialog.tsx
git commit -m "feat(tts): 新增选择合成模型弹窗（TtsModelDialog）"
```

---

## Task 5: 前端 — `TtsModelSwitchMenu` + 组件测试（TDD）

**Files:**
- Create: `src-tauri/frontend/src/components/models/TtsModelSwitchMenu.tsx`
- Test: `src-tauri/frontend/src/components/models/TtsModelSwitchMenu.test.tsx`

**Step 1: 写失败测试**

`src-tauri/frontend/src/components/models/TtsModelSwitchMenu.test.tsx`（镜像 `AsrModelSwitchMenu.test.tsx`；注意 tts 切片形状是 `tts.config.model_dir`（单层 config），与 asr 的 `asr.config.config.model_dir` 不同）：

```tsx
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TtsModelSwitchMenu } from "./TtsModelSwitchMenu";

// stub 选择合成模型弹窗：只记录 props，避免其内部（toast/invoke）整链依赖。
const { dialogProps } = vi.hoisted(() => ({
  dialogProps: { last: null as { open: boolean; onClose: () => void } | null },
}));

vi.mock("@/components/tts/TtsModelDialog", () => ({
  TtsModelDialog: (props: { open: boolean; onClose: () => void }) => {
    dialogProps.last = props;
    return props.open ? <div data-testid="tts-dialog">选择合成模型弹窗</div> : null;
  },
}));

// mock runtime：tts 切片可变（model_dir）。
const { state, navProbe } = vi.hoisted(() => ({
  state: {
    tts: null as { config: { model_dir: string; models_present: boolean } | null } | null,
  },
  // 模拟浏览器原生行为：a 内嵌 button 点击的默认动作是跟随祖先 href；
  // jsdom 不实现该行为，这里以 defaultPrevented 为准计数「原生导航」次数。
  navProbe: { count: 0 },
}));

vi.mock("@/providers/RuntimeContext", () => ({
  useRuntime: () => ({ tts: state.tts }),
}));

function makeTtsConfig(modelDir?: string) {
  state.tts = {
    config: {
      model_dir: modelDir ?? "/home/user/.zapmomo/models/sherpa-onnx-zipvoice-distill-int8-zh-en-emilia",
      models_present: true,
    },
  };
}

/** 挂真实链接行验证「不触发导航」；location 探针放在 /models。 */
function Probe() {
  const location = useLocation();
  return (
    <>
      <div data-testid="location">{location.pathname}</div>
      <a
        href="/models/tts"
        data-testid="row-link"
        onClick={(e) => {
          // 模拟原生「激活祖先 a」：拦截层调用了 preventDefault 则视为已阻止导航。
          if (!e.defaultPrevented) navProbe.count++;
        }}
      >
        <TtsModelSwitchMenu />
      </a>
    </>
  );
}

function renderMenu() {
  return render(
    <MemoryRouter initialEntries={["/models"]}>
      <Routes>
        <Route path="/models" element={<Probe />} />
        <Route path="/models/tts" element={<div>配置页</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  dialogProps.last = null;
  navProbe.count = 0;
  makeTtsConfig();
});

describe("TtsModelSwitchMenu 模型快速切换（弹窗版）", () => {
  it("模型名文本 + 「选择模型」按钮", () => {
    renderMenu();
    expect(
      screen.getByText("sherpa-onnx-zipvoice-distill-int8-zh-en-emilia"),
    ).toBeInTheDocument();
    const button = screen.getByRole("button", { name: "选择合成模型" });
    expect(button).toHaveTextContent("选择模型");
  });

  it("点击切换按钮打开选择合成模型弹窗", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(screen.getByRole("button", { name: "选择合成模型" }));

    expect(dialogProps.last?.open).toBe(true);
    expect(screen.getByTestId("tts-dialog")).toBeInTheDocument();
  });

  it("弹窗 onClose 回调关闭后可再次打开", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(screen.getByRole("button", { name: "选择合成模型" }));
    expect(dialogProps.last?.open).toBe(true);

    // onClose 触发 setState，等待 stub 以 open=false 重渲染。
    act(() => dialogProps.last?.onClose());
    await waitFor(() => expect(dialogProps.last?.open).toBe(false));

    await user.click(screen.getByRole("button", { name: "选择合成模型" }));
    await waitFor(() => expect(dialogProps.last?.open).toBe(true));
  });

  it("点击行内按钮/弹窗不触发所在行的链接导航（含原生 href 默认行为）", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(screen.getByRole("button", { name: "选择合成模型" }));
    expect(screen.getByTestId("tts-dialog")).toBeInTheDocument();
    expect(screen.getByTestId("location")).toHaveTextContent("/models");
    // 回归：拦截层必须 preventDefault，否则浏览器原生跟随 <a href> 整页跳转。
    expect(navProbe.count).toBe(0);
  });
});
```

**Step 2: 跑测试确认失败**

```bash
cd src-tauri/frontend && pnpm test:run src/components/models/TtsModelSwitchMenu.test.tsx
```
预期：FAIL（模块 `./TtsModelSwitchMenu` 不存在）。

**Step 3: 实现组件**

`src-tauri/frontend/src/components/models/TtsModelSwitchMenu.tsx`：

```tsx
import { FolderOpen } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { TtsModelDialog } from "@/components/tts/TtsModelDialog";
import { useRuntime } from "@/providers/RuntimeContext";

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/**
 * 摘要 TTS 行的模型快速切换：模型名文本 + 「选择模型」按钮（与 KWS/ASR/LLM 行
 * 同款样式/文案/交互），打开同一个选择合成模型弹窗。
 * 组件自带点击冒泡拦截（按钮/弹窗内点击均不触发所在行的 Link 导航）。
 */
export function TtsModelSwitchMenu() {
  const { tts } = useRuntime();
  const [open, setOpen] = useState(false);

  return (
    // 拦截点击：stopPropagation 挡 react-router 的 JS 导航，preventDefault 取消浏览器
    // 「激活祖先 <a>」的原生默认行为（a 内嵌 button 时点击会跟随 href 整页跳转）。
    // biome-ignore lint/a11y/noStaticElementInteractions: 静态容器仅拦截鼠标冒泡，交互由内部按钮承载
    // biome-ignore lint/a11y/useKeyWithClickEvents: 仅拦截点击冒泡防误触导航，键盘交互由内部按钮处理
    <span
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
      }}
    >
      <span className="inline-flex min-w-0 items-center gap-1.5">
        <span className="truncate text-xs text-text-secondary">
          {basename(tts.config?.model_dir ?? "")}
        </span>
        <Button size="sm" onClick={() => setOpen(true)} aria-label="选择合成模型">
          <FolderOpen className="h-4 w-4" />
          选择模型
        </Button>
      </span>
      <TtsModelDialog open={open} onClose={() => setOpen(false)} />
    </span>
  );
}
```

**Step 4: 跑测试确认通过**

```bash
cd src-tauri/frontend && pnpm test:run src/components/models/TtsModelSwitchMenu.test.tsx && pnpm exec tsc -b
```
预期：PASS。

**Step 5: Commit**

```bash
git add src-tauri/frontend/src/components/models/TtsModelSwitchMenu.tsx src-tauri/frontend/src/components/models/TtsModelSwitchMenu.test.tsx
git commit -m "feat(tts): 模型摘要行新增 TTS 选择模型快捷入口"
```

---

## Task 6: 前端 — `ModelSummary` TTS 行接入 + 测试更新

**Files:**
- Modify: `src-tauri/frontend/src/components/models/ModelSummary.tsx:255`（TTS 行 model 列）与 :26（`basename` 仅此行使用，随之删除）
- Modify: `src-tauri/frontend/src/components/models/ModelSummary.test.tsx`（stub `TtsModelDialog`）

**Step 1: 更新测试（先加 stub）**

在 `ModelSummary.test.tsx` 的 AsrModelDialog stub（:29-31）之后追加同款：

```tsx
// TTS 选择模型弹窗同理：stub 避免 useTtsModelSwitch 的 useToast/invoke 依赖。
vi.mock("@/components/tts/TtsModelDialog", () => ({
  TtsModelDialog: () => null,
}));
```

**Step 2: 跑现有测试确认仍通过（回归基线）**

```bash
cd src-tauri/frontend && pnpm test:run src/components/models/ModelSummary.test.tsx
```
预期：PASS（stub 尚未被引用，无副作用）。

**Step 3: 实现**

`ModelSummary.tsx`：
1. import 区加 `import { TtsModelSwitchMenu } from "./TtsModelSwitchMenu";`
2. TTS 行（:255）：`model: ttsConfigured ? basename(tts.config?.model_dir ?? "") : "未配置模型"` → `model: ttsConfigured ? <TtsModelSwitchMenu /> : "未配置模型"`
3. 删除 :26 的 `basename` 函数（改后无引用；若其他行仍引用则保留——实施时以 tsc/biome 无 unused 判定为准）。

**Step 4: 跑测试 + 类型检查**

```bash
cd src-tauri/frontend && pnpm test:run src/components/models/ModelSummary.test.tsx && pnpm exec tsc -b
```
预期：PASS。

**Step 5: Commit**

```bash
git add src-tauri/frontend/src/components/models/ModelSummary.tsx src-tauri/frontend/src/components/models/ModelSummary.test.tsx
git commit -m "feat(tts): 模型摘要 TTS 行接入选择模型弹窗"
```

---

## Task 7: 前端 — `TtsPage` / `TtsBasicConfig` 接入 + 测试更新

**Files:**
- Modify: `src-tauri/frontend/src/pages/models/TtsPage.tsx`
- Modify: `src-tauri/frontend/src/components/tts/TtsBasicConfig.tsx`
- Test: `src-tauri/frontend/src/pages/models/TtsPage.test.tsx`（更新「未下载模型」用例 + invoke mock）

**Step 1: 更新测试（改期望）**

`TtsPage.test.tsx`：
1. invoke mock 的 switch 中删除 `case "download_tts_model": …` 分支；参考 `AsrPage.test.tsx:117,193` 增加最小形状桩：

```ts
    case "list_model_library":
      return Promise.resolve([]);
```

（TtsPage 挂载即渲染 TtsModelDialog → useTtsModelSwitch mount 时调 `list_model_library`。）
2. 「未下载模型」用例（:254）断言更新：原「下载可用」改为存在「选择合成模型」按钮（`getByRole("button", { name: "选择合成模型" })`），点击可打开弹窗；「测试禁用」断言保留。
3. 若 useTtsModelSwitch 的 useToast 需要 Provider：参考 `AsrPage.test.tsx` 顶部的 Provider/mock 处理方式照搬。

**Step 2: 跑测试确认失败**

```bash
cd src-tauri/frontend && pnpm test:run src/pages/models/TtsPage.test.tsx
```
预期：FAIL（「选择合成模型」按钮不存在）。

**Step 3: 实现**

1. `TtsPage.tsx`：

```tsx
import { TtsModelDialog } from "@/components/tts/TtsModelDialog";
…
  const [modelDialogOpen, setModelDialogOpen] = useState(false);
…
      <TtsBasicConfig
        onTestOpen={() => setTestOpen(true)}
        onManageVoices={() => setVoicesOpen(true)}
        onOpenModelDialog={() => setModelDialogOpen(true)}
      />
…
      <TtsModelDialog open={modelDialogOpen} onClose={() => setModelDialogOpen(false)} />
```

2. `TtsBasicConfig.tsx`：
   - props 加 `onOpenModelDialog: () => void;`
   - destructure 移除 `downloading / downloadProgress / downloadError / download`；删除 `percent` / `busy` 计算与 `downloadProgress` / `downloadError` 两个展示块；import 移除 `Download`、`Progress`（`FolderOpen` 已有）。
   - 「模型文件缺失」Alert 文案改为：`模型文件缺失（{config.model_dir}）。点击下方「选择模型」重新下载或换用已安装模型。`
   - 底部按钮组：删除 `{!modelsPresent && (<Button onClick={download}…>下载模型</Button>)}`，新增（始终显示、放在「测试语音」前）：

```tsx
        <Button onClick={onOpenModelDialog}>
          <FolderOpen className="h-4 w-4" />
          选择模型
        </Button>
```

**Step 4: 跑测试确认通过**

```bash
cd src-tauri/frontend && pnpm test:run src/pages/models/TtsPage.test.tsx && pnpm exec tsc -b
```
预期：PASS。

**Step 5: Commit**

```bash
git add src-tauri/frontend/src/pages/models/TtsPage.tsx src-tauri/frontend/src/components/tts/TtsBasicConfig.tsx src-tauri/frontend/src/pages/models/TtsPage.test.tsx
git commit -m "feat(tts): 配置页「下载模型」替换为「选择模型」弹窗入口"
```

---

## Task 8: 前端 — 移除 legacy 一键下载通道（useTts / tauri.ts）

**Files:**
- Modify: `src-tauri/frontend/src/hooks/useTts.ts`
- Modify: `src-tauri/frontend/src/lib/tauri.ts`（删 `downloadTtsModel` / `onTtsDownloadProgress` 封装）

**Step 1: 清理 `useTts.ts`**

- `TtsState` 接口删 `downloading / downloadProgress / downloadError / download` 四个成员（:45-48）。
- 删对应 3 个 `useState`（:67-69）、`onTtsDownloadProgress` 订阅项（:122）及 import（:4）、`download` callback（:208-221）及 return 中 4 个字段（:249-252）。
- `DownloadProgress` 类型 import（:11）随之删除。
- `refreshVoices` 保留（音色管理仍用）；`refreshConfig` 保留。

**Step 2: 清理 `lib/tauri.ts`**

- 删 `downloadTtsModel` 与 `onTtsDownloadProgress` 封装及其事件名常量（若为 tts 专用）。
- 先 `grep -rn "downloadTtsModel\|onTtsDownloadProgress" src-tauri/frontend/src` 确认仅 tauri.ts 自身（Task 7 后 useTts 已不引用）。
- 后端 `download_tts_model` command **保留不动**（`src-tauri/src/lib.rs:1115`，不在本期清理范围）。

**Step 3: 全量前端检查**

```bash
cd src-tauri/frontend && pnpm exec tsc -b && pnpm test:run && pnpm check
```
预期：全部通过（vitest 全量 + biome）。

**Step 4: Commit**

```bash
git add src-tauri/frontend/src/hooks/useTts.ts src-tauri/frontend/src/lib/tauri.ts
git commit -m "refactor(tts): 移除前端 legacy 一键下载通道，统一走模型库弹窗"
```

---

## Task 9: 全量验证（不产生 commit）

**Step 1: Rust 完整检查**

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test -- --test-threads=1
cargo check -p zapmomo-app
```
预期：全部通过。

**Step 2: 前端完整检查**

```bash
cd src-tauri/frontend && pnpm exec tsc -b && pnpm test:run && pnpm check
```
预期：全部通过。

**Step 3: 手工验收（可选，需 GUI）**

`pnpm tauri dev` 后按设计文档 §5 验收清单逐项检查（弹窗三态 / 下载进度 / 设为当前 / 卸载保护 / 老用户已装互通）。

---

## 回滚策略

每个 Task 独立 commit，可按 commit 粒度 revert。后端 `download_tts_model` command 未动，回滚前端后 legacy 通道可由后端恢复（前端封装需 revert Task 8）。
