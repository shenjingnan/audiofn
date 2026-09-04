import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import type { KwsConfigInfo, KwsParamsPatch } from "@/types/tauri";

export interface KwsConfigState {
  config: KwsConfigInfo | null;
  error: string | null;
  refresh: () => Promise<void>;
  /** 持久化「启用 KWS」偏好（[kws].enabled），写成功后回读配置。 */
  setEnabled: (enabled: boolean) => Promise<void>;
  /** 持久化 KWS 引擎/运行参数（[kws] 批写入），写成功后回读配置。 */
  setParams: (patch: KwsParamsPatch) => Promise<void>;
}

/** 读取 KWS 配置与模型状态。 */
export function useKwsConfig(): KwsConfigState {
  const [config, setConfig] = useState<KwsConfigInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setConfig(await api.getKwsConfig());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const setEnabled = useCallback(
    async (enabled: boolean) => {
      try {
        await api.setKwsEnabled({ enabled });
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh],
  );

  const setParams = useCallback(
    async (patch: KwsParamsPatch) => {
      // 保存失败向上抛出，由调用方（高级参数表单）展示内联错误
      await api.setKwsParams({ params: patch });
      await refresh();
    },
    [refresh],
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { config, error, refresh, setEnabled, setParams };
}
