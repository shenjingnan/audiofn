import { CircleAlert, Mic, Square } from "lucide-react";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { onAsrVadDownloadProgress } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";
import { ASR_STATUS_COLOR, asrDictateStatus } from "./asrMeta";

/**
 * 免提连续听写面板（离线 SenseVoice/Whisper 模型专用）：
 * 开始/停止听写 + 逐句展示 VAD 分段整句转写结果（最新在上，最新段高亮）。
 * 首次听写自动下载 Silero VAD 模型（约 0.6MB）。
 */
export function AsrDictatePanel() {
  const { asr, device } = useRuntime();
  const { isDictating, pending, error, start, stop } = asr.dictate;
  const { segments } = asr.dictateResults;
  const vadPresent = asr.config.config?.vad_present ?? false;
  const status = asrDictateStatus(asr.dictate);
  const newestId = segments[0]?.id;
  const [vadProgress, setVadProgress] = useState<string | null>(null);

  // 首次听写自动下载 VAD 模型：跟踪进度，完成后刷新配置（vad_present → true）
  useEffect(() => {
    const unlisten = onAsrVadDownloadProgress((p) => {
      if (p.stage === "done") {
        setVadProgress(null);
        void asr.config.refresh();
      } else {
        setVadProgress(p.message);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [asr.config]);

  const handleToggle = (on: boolean) => {
    if (on) void start(device || null);
    else void stop();
  };

  return (
    <section className="rounded-[16px] border border-panel-border bg-panel-background">
      <div className="flex items-center justify-between gap-3 border-b border-divider px-3.5 py-3">
        <span className="flex items-center gap-2.5">
          <span
            className={cn(
              "inline-flex items-center gap-1.5 text-sm font-medium",
              ASR_STATUS_COLOR[status.tone],
            )}
          >
            <span className="h-1.5 w-1.5 rounded-full bg-current" />
            {status.label}
          </span>
          <h2 className="text-base font-semibold text-text-primary">免提连续听写</h2>
        </span>
        <Button size="sm" disabled={pending} onClick={() => handleToggle(!isDictating)}>
          {isDictating ? <Square className="h-4 w-4" /> : <Mic className="h-4 w-4" />}
          {isDictating ? "停止听写" : "开始听写"}
        </Button>
      </div>

      <div className="space-y-2 px-3.5 py-3">
        {vadProgress ? (
          <Alert variant="warning">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription>正在下载 VAD 模型：{vadProgress}</AlertDescription>
          </Alert>
        ) : !vadPresent ? (
          <Alert variant="warning">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription>首次听写将自动下载 Silero VAD 模型（约 0.6MB）。</AlertDescription>
          </Alert>
        ) : null}

        {error && (
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{error}</AlertDescription>
          </Alert>
        )}

        {!isDictating && segments.length === 0 ? (
          <p className="text-xs text-text-muted">
            说一句话，停顿后自动转写整句并显示在这里（SenseVoice/Whisper 离线模型专用）。
          </p>
        ) : (
          <ul className="max-h-64 space-y-1 overflow-y-auto">
            {segments.map((s) => (
              <li
                key={s.id}
                className={cn(
                  "rounded-md border border-panel-border bg-app-background/60 px-3 py-2",
                  isDictating && s.id === newestId && "border-emerald-500/40",
                )}
              >
                <div className="flex items-start justify-between gap-3">
                  <span className="min-w-0 flex-1 text-sm text-text-primary">{s.text}</span>
                  <span className="shrink-0 text-xs text-text-muted">{s.at}</span>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
