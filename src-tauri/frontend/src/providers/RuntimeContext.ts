import { createContext, useContext } from "react";
import type { AsrConfigState } from "@/hooks/useAsrConfig";
import type { AsrDictateState } from "@/hooks/useAsrDictate";
import type { AsrDictateResultsState } from "@/hooks/useAsrDictateResults";
import type { AsrModelDownloadState } from "@/hooks/useAsrModelDownload";
import type { DevicesState } from "@/hooks/useDevices";
import type { TtsState } from "@/hooks/useTts";
import type { AppInfo } from "@/types/tauri";

/** 全局运行态：由 `AppRuntimeProvider` 集中提供，页面/卡片通过 `useRuntime()` 读取。 */
export interface RuntimeState {
  appInfo: AppInfo | null;
  devices: DevicesState;
  asr: {
    config: AsrConfigState;
    download: AsrModelDownloadState;
    /** 免提连续听写（离线模型，麦克风实时转写）运行状态 */
    dictate: AsrDictateState;
    /** 听写结果段 */
    dictateResults: AsrDictateResultsState;
  };
  tts: TtsState;
  /** 全局选中的麦克风设备（听写与 TTS 录音共用） */
  device: string;
  setDevice: (device: string) => void;
  /** 任一麦克风占用进行中（用于禁用设备切换/录音等） */
  anyListening: boolean;
}

export const RuntimeContext = createContext<RuntimeState | null>(null);

/** 读取全局运行态；必须在 `AppRuntimeProvider` 内使用。 */
export function useRuntime(): RuntimeState {
  const ctx = useContext(RuntimeContext);
  if (!ctx) {
    throw new Error("useRuntime 必须在 AppRuntimeProvider 内使用");
  }
  return ctx;
}
