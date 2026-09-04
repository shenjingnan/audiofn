import { useMemo } from "react";
import { CapabilityOverview } from "@/components/home/CapabilityOverview";
import { deriveOverview } from "@/components/home/overviewMeta";
import { useRuntime } from "@/providers/RuntimeContext";

/**
 * 概览页：AI 能力状态（ASR / TTS 两卡）。
 *
 * 状态全部读取真实业务数据（useRuntime），页面本身无独立状态源。
 */
export function HomePage() {
  const { asr, tts } = useRuntime();

  const statuses = useMemo(() => deriveOverview({ asr, tts }), [asr, tts]);

  return (
    <div className="flex h-full flex-col gap-4 overflow-hidden">
      <div>
        <h1 className="text-xl font-semibold tracking-tight text-text-primary">概览</h1>
        <p className="mt-0.5 text-sm text-muted-foreground">查看语音识别与语音合成的状态</p>
      </div>

      <div className="grid min-h-0 flex-1 gap-4">
        <CapabilityOverview statuses={statuses} />
      </div>
    </div>
  );
}
