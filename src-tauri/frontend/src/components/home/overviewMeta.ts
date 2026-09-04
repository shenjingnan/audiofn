import { type LucideIcon, Mic, Volume2 } from "lucide-react";
import { deriveListenerStatus, type ListenerKind } from "@/components/models/capabilityStatus";
import type { TtsState } from "@/hooks/useTts";
import type { RuntimeState } from "@/providers/RuntimeContext";

/** 状态语义色（与模型页 ModelSummary / 各能力 meta 的语义完全一致）。 */
export type OverviewTone = "good" | "idle" | "loading" | "warn" | "error";

export const OVERVIEW_STATUS_COLOR: Record<OverviewTone, string> = {
  good: "text-emerald-600",
  idle: "text-text-muted",
  loading: "text-blue-600",
  warn: "text-amber-600",
  error: "text-red-600",
};

/** AI 能力小卡数据（纯展示：Icon + 名称 + 缩写 + 状态）。 */
export interface CapabilityStatus {
  key: "asr" | "tts";
  name: string;
  code: string;
  icon: LucideIcon;
  accent: string;
  label: string;
  tone: OverviewTone;
}

export interface OverviewInput {
  asr: RuntimeState["asr"];
  tts: TtsState;
}

/** 监听型能力 kind → 概览页文案（listening 态展示「识别中」）。 */
function listenerLabel(kind: ListenerKind, active: "监听中" | "识别中"): string {
  switch (kind) {
    case "error":
      return "异常";
    case "starting":
      return "启动中";
    case "listening":
      return active;
    case "ready":
      return "已就绪";
    case "disabled":
      return "未启用";
    case "not_configured":
      return "未配置";
  }
}

/** ASR 状态：错误 > 启动中 > 识别中 > 已就绪/未启用 > 未配置（读取持久化 enabled）。 */
function asrStatus(asr: RuntimeState["asr"]): { label: string; tone: OverviewTone } {
  const st = deriveListenerStatus({
    error: asr.listening.error,
    pending: asr.listening.pending,
    isListening: asr.listening.isListening,
    enabled: asr.config.config?.enabled,
    modelsPresent: asr.config.config?.models_present,
  });
  return { label: listenerLabel(st.kind, "识别中"), tone: st.tone };
}

/** TTS 状态：配置错误 > 合成中 > 未配置 > 已关闭 > 已就绪（顺序沿用 ttsMeta：模型缺失优先于已关闭）。 */
function ttsStatus(tts: TtsState): { label: string; tone: OverviewTone } {
  if (tts.configError) return { label: "异常", tone: "error" };
  if (tts.synthesizing) return { label: "合成中", tone: "loading" };
  const cfg = tts.config;
  if (!cfg) return { label: "加载中", tone: "idle" };
  if (!cfg.models_present) return { label: "未配置", tone: "idle" };
  if (cfg.enabled === false) return { label: "已关闭", tone: "idle" };
  return { label: "已就绪", tone: "good" };
}

/**
 * 概览页 AI 能力状态推导（纯函数）：基于真实 runtime 字段推导，
 * 不维护第二套状态源。顺序固定为 ASR / TTS（与模型摘要一致）。
 */
export function deriveOverview(input: OverviewInput): CapabilityStatus[] {
  const { asr, tts } = input;
  const asrState = asrStatus(asr);
  const ttsState = ttsStatus(tts);

  return [
    {
      key: "asr",
      name: "语音识别",
      code: "ASR",
      icon: Mic,
      accent: "bg-blue-100 text-blue-600",
      ...asrState,
    },
    {
      key: "tts",
      name: "语音合成",
      code: "TTS",
      icon: Volume2,
      accent: "bg-amber-100 text-amber-600",
      ...ttsState,
    },
  ];
}
