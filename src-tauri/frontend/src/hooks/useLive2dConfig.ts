import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import type { Live2dConfigInfo } from "@/types/tauri";

export interface Live2dConfigState {
  config: Live2dConfigInfo | null;
  error: string | null;
  refresh: () => Promise<void>;
}

/** 读取 Live2D 配置与模型状态（用于启动时恢复持久化的模型预览）。 */
export function useLive2dConfig(): Live2dConfigState {
  const [config, setConfig] = useState<Live2dConfigInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setConfig(await api.getLive2dConfig());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { config, error, refresh };
}
