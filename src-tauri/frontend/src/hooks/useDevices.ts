import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/tauri";

export interface DevicesState {
  devices: string[];
  error: string | null;
  refresh: () => Promise<void>;
}

/** 读取麦克风输入设备列表，支持手动刷新。 */
export function useDevices(): DevicesState {
  const [devices, setDevices] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setDevices(await api.listDevices());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { devices, error, refresh };
}
