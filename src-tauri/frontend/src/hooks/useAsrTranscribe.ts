import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { api } from "@/lib/tauri";
import type { TranscribeResult } from "@/types/tauri";

/**
 * 文件转写：选择 wav → `transcribe_audio`（后端整段转写当前识别模型）。
 * 状态：转写中 / 结果 / 错误；供「转写文件」弹窗使用。
 * 收录模型为 raw 单 GGUF、不带示例音频，转写必须由用户选择音频文件。
 */
export function useAsrTranscribe() {
  const [transcribing, setTranscribing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<TranscribeResult | null>(null);

  const transcribe = async (wavPath: string) => {
    setTranscribing(true);
    setError(null);
    setResult(null);
    try {
      setResult(await api.transcribeAudio({ wavPath }));
    } catch (e) {
      setError(String(e));
    } finally {
      setTranscribing(false);
    }
  };

  const pickAndTranscribe = async () => {
    const path = await open({
      multiple: false,
      title: "选择要转写的音频（WAV）",
      filters: [{ name: "WAV", extensions: ["wav"] }],
    });
    if (typeof path !== "string") return; // 用户取消对话框
    await transcribe(path);
  };

  const clear = () => {
    setResult(null);
    setError(null);
  };

  return { pickAndTranscribe, transcribing, error, result, clear };
}
