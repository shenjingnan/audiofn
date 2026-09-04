import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import type { SystemResources } from "@/types/modelLibrary";

/**
 * 独立的系统资源轮询 hook（不依赖模型库状态）。
 *
 * AppShell 始终挂载，因此顶部状态栏可持续刷新。
 * - 页面可见时每 30s 自动刷新
 * - 页面不可见时暂停轮询
 * - 轮询失败静默忽略（不弹 toast）
 */
export function useSystemResources() {
  const [resources, setResources] = useState<SystemResources | null>(null);

  const refresh = useCallback(async () => {
    try {
      setResources(await api.getSystemResources());
    } catch {
      // 静默：轮询失败下次再试
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => {
      if (document.visibilityState === "visible") {
        void refresh();
      }
    }, 30_000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  return { resources, refresh };
}
