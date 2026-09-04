import { type ReactNode, useCallback, useEffect, useState } from "react";
import { useToast } from "@/components/ui/toast";
import { useAppInfo } from "@/hooks/useAppInfo";
import { useAsrConfig } from "@/hooks/useAsrConfig";
import { useAsrDictate } from "@/hooks/useAsrDictate";
import { useAsrDictateResults } from "@/hooks/useAsrDictateResults";
import { useAsrModelDownload } from "@/hooks/useAsrModelDownload";
import { useDevices } from "@/hooks/useDevices";
import { useTts } from "@/hooks/useTts";
import { api } from "@/lib/tauri";
import { RuntimeContext, type RuntimeState } from "./RuntimeContext";

/**
 * 运行态 Provider：把 ASR / TTS 的 hooks 集中在此调用，并常驻于路由外层
 * （`<Routes>` 之外），使下载/听写/合成状态不随页面切换丢失。
 * Router 只负责「当前显示哪个 UI」，不决定 runtime 生命周期。
 */
export function AppRuntimeProvider({ children }: { children: ReactNode }) {
  const toast = useToast();
  const appInfo = useAppInfo();
  const devices = useDevices();
  const asrConfig = useAsrConfig();
  const asrDownload = useAsrModelDownload(asrConfig.refresh);
  const asrDictate = useAsrDictate();
  const asrDictateResults = useAsrDictateResults();
  const tts = useTts();

  // 麦克风选择：跨页面全局共享（听写/转写/TTS 录音均消费），持久化到 backend settings.toml（顶层 microphone）。
  // 启动时回读后端；旧版本遗留的 localStorage 记忆做一次性迁移（读后即清，仅在读成功后才清理）。
  const [device, setDeviceState] = useState("");
  useEffect(() => {
    let cancelled = false;
    (async () => {
      let saved: string;
      try {
        saved = await api.getMicrophone();
      } catch {
        return; // 读取失败：保持默认，不迁移不清理
      }
      if (cancelled) return;
      if (saved) {
        setDeviceState(saved);
      } else {
        const legacy = localStorage.getItem("audiofn.microphone");
        if (legacy) {
          setDeviceState(legacy);
          void api.setMicrophone({ mic: legacy }).catch(() => {});
        }
      }
      localStorage.removeItem("audiofn.microphone");
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const setDevice = useCallback(
    (d: string) => {
      setDeviceState(d);
      // 听写/录音中切换会触发后端用新设备重启采集；失败（如新设备不可用）时提示原因。
      void api.setMicrophone({ mic: d }).catch((e) => toast.error(String(e)));
    },
    [toast],
  );

  // 设备列表就绪后校验记忆的设备是否仍存在（如外设拔出），否则清空避免 start 时按不存在设备报错
  useEffect(() => {
    if (device && devices.devices.length > 0 && !devices.devices.includes(device)) {
      setDevice("");
    }
  }, [device, devices.devices, setDevice]);

  const anyListening = asrDictate.isDictating;

  const value: RuntimeState = {
    appInfo,
    devices,
    asr: {
      config: asrConfig,
      download: asrDownload,
      dictate: asrDictate,
      dictateResults: asrDictateResults,
    },
    tts,
    device,
    setDevice,
    anyListening,
  };

  return <RuntimeContext.Provider value={value}>{children}</RuntimeContext.Provider>;
}
