/** 状态语义色：绿=识别中、蓝=启动中、灰=未识别、红=错误。 */
export type AsrStatusTone = "good" | "loading" | "idle" | "error";

export const ASR_STATUS_COLOR: Record<AsrStatusTone, string> = {
  good: "text-emerald-600",
  loading: "text-blue-600",
  idle: "text-text-muted",
  error: "text-red-600",
};

/** 模型类型徽标文案（选择模型弹窗 / 流式参数隐藏判断共用）。 */
export function asrModelKindLabel(kind: string): string {
  switch (kind) {
    case "zipformer":
      return "流式 Zipformer";
    case "paraformer":
      return "流式 Paraformer";
    case "sensevoice":
      return "SenseVoice";
    case "whisper":
      return "Whisper";
    case "qwen3_asr":
      return "Qwen3-ASR";
    default:
      return "ASR";
  }
}

/** 是否流式模型（zipformer/paraformer 走实时识别；其余为离线，仅支持转写文件）。 */
export function isStreamingAsr(kind: string | null | undefined): boolean {
  return kind === "zipformer" || kind === "paraformer" || !kind; // 缺省视为 zipformer（老配置无 model_type）
}

/** 离线听写状态机（判断顺序：错误 > 启动中 > 听写中 > 未听写）。 */
export function asrDictateStatus(st: {
  isDictating: boolean;
  pending: boolean;
  error: string | null;
}): { tone: AsrStatusTone; label: string } {
  if (st.error) return { tone: "error", label: "错误" };
  if (st.pending) return { tone: "loading", label: "启动中" };
  if (st.isDictating) return { tone: "good", label: "听写中" };
  return { tone: "idle", label: "未听写" };
}

/** 从 model_dir 派生展示名：取 basename。空路径返回 null，不硬编码任何模型名。 */
export function modelNameFromDir(dir: string | null | undefined): string | null {
  if (!dir) return null;
  return dir.split(/[\\/]/).pop() ?? dir;
}
