import { useCallback, useEffect, useRef, useState } from "react";
import { useToast } from "@/components/ui/toast";
import { api, onModelLibraryDownloadProgress } from "@/lib/tauri";
import { useRuntime } from "@/providers/RuntimeContext";
import { useStorageGate } from "@/providers/StorageGateProvider";
import type { LibraryModel, ModelLibraryProgress, SetCurrentResult } from "@/types/modelLibrary";

/**
 * ASR 切换弹窗的内置预设（id = models/model_registry.json 的 registry id）。
 * 一期模型库只收录 Qwen3-ASR 一条（zipformer 流式族已随 sherpa 后端移除，
 * 不再作为预设）；registry 该条目无 `platforms` 约束 = 全平台可见。
 */
export const ASR_PRESETS = [
  {
    id: "asr-qwen3-0.6b-audiocpp",
    name: "Qwen3-ASR 0.6B (audio.cpp)",
    tagline: "30 语言自动识别 · GPU 加速（macOS Metal / Linux CPU）· 不支持热词 · 包体约 1.1GB",
    sizeBytes: 1_151_272_416,
    kind: "qwen3_asr",
  },
] as const;

export interface AsrModelSwitchState {
  /** `list_model_library` 快照（含安装 / current 状态）；null = 尚未加载 */
  models: LibraryModel[] | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  /** 下载 registry 模型（model-library-download-progress 进度） */
  download: (id: string) => Promise<void>;
  downloadingId: string | null;
  progress: ModelLibraryProgress | null;
  /** 设为当前模型；听写中切换后自动 stop → start 重启听写使新模型立即生效 */
  setCurrent: (id: string) => Promise<void>;
  /** 卸载（managed 删文件；当前/运行中模型后端会拒绝） */
  remove: (id: string) => Promise<void>;
}

/**
 * ASR 模型切换状态：从后端模型列表过滤 ASR 条目，提供下载 / 设为当前 / 卸载。
 * 数据用 `list_model_library`（后端模型列表真相源，含 install_state + current）。
 */
export function useAsrModelSwitch(): AsrModelSwitchState {
  const runtime = useRuntime();
  const toast = useToast();
  const gate = useStorageGate();
  const [models, setModels] = useState<LibraryModel[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [progress, setProgress] = useState<ModelLibraryProgress | null>(null);
  /** 下载终态（done/cancelled/failed）：await 返回时事件可能尚未到达，用 ref 透传 */
  const terminalStage = useRef<string | null>(null);

  // setCurrent 的 await 期间 runtime 可能变化（重启识别需读最新 device/isListening）
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
      // 首次下载引导（选存储位置）；用户取消则静默中止（不置忙碌态）
      if (!(await gate.ensureStorageReady())) return;
      setDownloadingId(id);
      setProgress(null);
      terminalStage.current = null;
      try {
        await api.downloadLibraryModel({ id });
        const stage = terminalStage.current;
        if (stage === "cancelled") {
          toast.warning("已取消下载");
        } else {
          const name = ASR_PRESETS.find((p) => p.id === id)?.name ?? id;
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
    [gate, toast, refresh],
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
      await Promise.allSettled([runtimeRef.current.asr.config.refresh(), refresh()]);
      // 后端只写配置（restart_required）：听写中切换由前端重启听写使新模型立即生效
      // （与高级参数保存后的重启同款模式）
      const asr = runtimeRef.current.asr;
      if (res.runtimeAction === "restart_required" && asr.dictate.isDictating) {
        await asr.dictate.stop();
        await asr.dictate.start(runtimeRef.current.device || null);
        if (asr.dictate.error) {
          toast.error(`模型已切换，但重启听写失败：${asr.dictate.error}`);
        } else {
          toast.success("已切换模型并重启听写");
        }
      } else {
        toast.success(res.message);
      }
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
      await Promise.allSettled([runtimeRef.current.asr.config.refresh(), refresh()]);
    },
    [toast, refresh],
  );

  return { models, loading, error, refresh, download, downloadingId, progress, setCurrent, remove };
}
