import { useEffect, useState } from "react";
import { api, onAsrDictateStarted, onAsrDictateStopped } from "@/lib/tauri";

export interface AsrDictateState {
  isDictating: boolean;
  /** start/stop 在途标志 */
  pending: boolean;
  error: string | null;
  start: (device: string | null) => Promise<void>;
  stop: () => Promise<void>;
}

/**
 * 免提听写状态管理：初始化时回读后端状态，订阅 `asr-dictate-started/stopped` 事件；
 * start/stop 包装对应 command 并同步 UI 状态与错误。
 */
export function useAsrDictate(): AsrDictateState {
  const [isDictating, setIsDictating] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .isAsrDictating()
      .then(setIsDictating)
      .catch(() => {});

    const unlisten = onAsrDictateStopped((payload) => {
      setIsDictating(false);
      if (payload.error) setError(payload.error);
    });
    const unlistenStarted = onAsrDictateStarted(() => {
      setIsDictating(true);
      setError(null);
    });

    return () => {
      unlisten.then((fn) => fn());
      unlistenStarted.then((fn) => fn());
    };
  }, []);

  const start = async (device: string | null) => {
    setPending(true);
    setError(null);
    try {
      await api.startAsrDictate({ device });
      setIsDictating(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  };

  const stop = async () => {
    setPending(true);
    try {
      await api.stopAsrDictate();
      setIsDictating(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  };

  return { isDictating, pending, error, start, stop };
}
