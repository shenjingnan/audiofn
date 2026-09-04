import { useEffect, useState } from "react";
import { api, onAsrStarted, onAsrStopped } from "@/lib/tauri";

export interface AsrListeningState {
  isListening: boolean;
  /** start/stop 在途标志：RunControl 与 TestDialog 共享的唯一 in-flight 状态（消除 command 已发到 isListening 落盘之间的重复点击窗口） */
  pending: boolean;
  error: string | null;
  start: (device: string | null) => Promise<void>;
  stop: () => Promise<void>;
}

/**
 * ASR 识别状态管理：初始化时回读后端状态，订阅 `asr-stopped` 事件；
 * start/stop 包装对应 command 并同步 UI 状态与错误。
 */
export function useAsrListening(): AsrListeningState {
  const [isListening, setIsListening] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .isAsrListening()
      .then(setIsListening)
      .catch(() => {});

    const unlisten = onAsrStopped((payload) => {
      setIsListening(false);
      if (payload.error) setError(payload.error);
    });
    const unlistenStarted = onAsrStarted(() => {
      setIsListening(true);
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
      await api.startAsrListen({ device });
      setIsListening(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  };

  const stop = async () => {
    setPending(true);
    try {
      await api.stopAsrListen();
      setIsListening(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  };

  return { isListening, pending, error, start, stop };
}
