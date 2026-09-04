// ASR/TTS 这类「配置型」能力的状态推导（单一来源）。
//
// 之前 ModelSummary 与 overviewMeta 各自推导，导致漂移（ModelSummary 忘了读
// enabled，概览 ASR 也没有「未启用」态），同一时刻「能力链路开启、摘要未启用」。
// 这里把判断顺序 + enabled 门控 + tone 集中成一份，各展示面只保留自己的文案映射。
//
// 语义优先级：错误 > 启动中(pending) > 运行中(听写进行中)
// > 已就绪(models_present && enabled) > 未启用(models_present && !enabled) > 未配置。
//
// enabled=true 但未在运行是合法状态（按需启动），此时展示「已就绪」而非「未启用」。

export type ListenerKind =
  | "error"
  | "starting"
  | "listening"
  | "ready"
  | "disabled"
  | "not_configured";

export type ListenerTone = "good" | "idle" | "loading" | "error";

export interface ListenerStatusInput {
  error: string | null;
  /** 启动中（start 在途） */
  pending?: boolean;
  /** 是否正在运行（ASR = 听写中） */
  isListening: boolean;
  enabled: boolean | undefined;
  modelsPresent: boolean | undefined;
}

export interface ListenerStatus {
  kind: ListenerKind;
  tone: ListenerTone;
}

export const LISTENER_TONE: Record<ListenerKind, ListenerTone> = {
  error: "error",
  starting: "loading",
  listening: "good",
  ready: "good",
  disabled: "idle",
  not_configured: "idle",
};

/** 推导监听型能力的状态（只返回语义 kind + tone，文案由各展示面自行映射）。 */
export function deriveListenerStatus(input: ListenerStatusInput): ListenerStatus {
  if (input.error) return { kind: "error", tone: LISTENER_TONE.error };
  if (input.pending) return { kind: "starting", tone: LISTENER_TONE.starting };
  if (input.isListening) return { kind: "listening", tone: LISTENER_TONE.listening };
  const configured = input.modelsPresent ?? false;
  if (configured) {
    return input.enabled
      ? { kind: "ready", tone: LISTENER_TONE.ready }
      : { kind: "disabled", tone: LISTENER_TONE.disabled };
  }
  return { kind: "not_configured", tone: LISTENER_TONE.not_configured };
}
