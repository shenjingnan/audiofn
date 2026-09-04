import { CircleAlert, Mic, Square } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";
import { ASR_STATUS_COLOR, asrDictateStatus } from "./asrMeta";

/**
 * 免提连续听写面板：开始/停止听写 + 逐句展示整句转写结果（最新在上，最新段高亮）。
 */
export function AsrDictatePanel() {
  const { asr, device } = useRuntime();
  const { isDictating, pending, error, start, stop } = asr.dictate;
  const { segments } = asr.dictateResults;
  const status = asrDictateStatus(asr.dictate);
  const newestId = segments[0]?.id;

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
        {error && (
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{error}</AlertDescription>
          </Alert>
        )}

        {!isDictating && segments.length === 0 ? (
          <p className="text-xs text-text-muted">说一句话，停顿后自动转写整句并显示在这里。</p>
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
