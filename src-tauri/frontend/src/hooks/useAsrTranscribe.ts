import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { api } from "@/lib/tauri";
import type { TranscribeResult } from "@/types/tauri";

/**
 * 一键离线转写：选择 wav → `transcribe_audio`（后端按 model_type 分发在线/离线引擎）。
 * 状态：转写中 / 结果 / 错误；供「转写文件」弹窗使用。
 * `runDefaultTest` 传 null 路径，转写模型自带 test_wavs 示例（离线「测试识别」）。
 */
export function useAsrTranscribe() {
  const [transcribing, setTranscribing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<TranscribeResult | null>(null);

  const transcribe = async (wavPath: string | null) => {
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

  const runDefaultTest = () => transcribe(null);

  const clear = () => {
    setResult(null);
    setError(null);
  };

  return { pickAndTranscribe, runDefaultTest, transcribing, error, result, clear };
}
