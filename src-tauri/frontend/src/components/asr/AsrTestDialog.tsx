import { CircleAlert, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";
import { modelNameFromDir } from "./asrMeta";

const EXIT_MS = 200;

interface AsrTestDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * 测试语音识别对话框。
 * 复用全局 `useRuntime().asr` 单实例 runtime，不创建第二套：
 * - 打开时若未在识别则自动 start_asr_listen 并记录 `startedByDialog`；
 * - 原本已在识别（顶部开关发起）则只展示实时结果，不重复 start；
 * - 关闭时仅停止「由本对话框发起」的识别，绝不停止顶部开关发起的会话。
 */
export function AsrTestDialog({ open, onClose }: AsrTestDialogProps) {
  const { asr, device } = useRuntime();
  const [mounted, setMounted] = useState(open);
  const [closing, setClosing] = useState(false);
  const startedByDialog = useRef(false);
  const autoStartHandled = useRef(false);
  const listeningRef = useRef(asr.listening);
  useEffect(() => {
    listeningRef.current = asr.listening;
  }, [asr.listening]);

  useEffect(() => {
    if (open) {
      setMounted(true);
      setClosing(false);
      startedByDialog.current = false;
      autoStartHandled.current = false;
    }
  }, [open]);

  useEffect(() => {
    if (!open || !mounted || closing || autoStartHandled.current) return;
    if (asr.listening.isListening) {
      autoStartHandled.current = true;
      return;
    }
    if (asr.listening.pending) return;
    autoStartHandled.current = true;
    startedByDialog.current = true;
    void asr.listening.start(device || null);
  }, [
    open,
    mounted,
    closing,
    asr.listening.isListening,
    asr.listening.pending,
    device,
    asr.listening.start,
  ]);

  const stopDialogListen = useCallback(async () => {
    while (listeningRef.current.pending) {
      await new Promise((r) => setTimeout(r, 50));
    }
    if (listeningRef.current.isListening) {
      await asr.listening.stop();
    }
  }, [asr.listening.stop]);

  const finishClose = useCallback(() => {
    const mine = startedByDialog.current;
    setMounted(false);
    setClosing(false);
    onClose();
    if (mine) void stopDialogListen();
  }, [onClose, stopDialogListen]);

  const close = useCallback(() => {
    if (closing) return;
    setClosing(true);
    window.setTimeout(finishClose, EXIT_MS);
  }, [closing, finishClose]);

  useEffect(() => {
    if (!mounted || closing) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [mounted, closing, close]);

  if (!mounted) return null;

  const { listening } = asr;
  const results = asr.results;
  const modelName = modelNameFromDir(asr.config.config?.model_dir);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-label="测试语音识别"
    >
      <button
        type="button"
        tabIndex={-1}
        aria-label="关闭对话框"
        className={cn(
          "absolute inset-0 cursor-default bg-black/20",
          closing ? "animate-out fade-out-0 duration-200" : "animate-in fade-in-0 duration-200",
        )}
        onClick={close}
      />
      <div
        className={cn(
          "relative flex max-h-[85vh] w-full max-w-xl flex-col rounded-xl border border-panel-border bg-panel-background",
          closing
            ? "animate-out fade-out-0 zoom-out-95 duration-200 ease-in"
            : "animate-in fade-in-0 zoom-in-95 duration-200 ease-out",
        )}
      >
        <div className="flex items-center justify-between gap-4 border-b border-divider px-5 py-4">
          <h3 className="text-sm font-semibold text-text-primary">测试语音识别</h3>
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8 shrink-0"
            onClick={close}
            aria-label="关闭"
          >
            <X className="h-4 w-4" />
          </Button>
        </div>

        <div className="flex-1 space-y-3 overflow-y-auto px-5 py-4">
          <div className="flex items-center gap-2">
            <span
              className={cn(
                "inline-flex items-center gap-1.5 text-sm font-medium",
                listening.error
                  ? "text-red-600"
                  : listening.isListening
                    ? "text-emerald-600"
                    : "text-text-muted",
              )}
            >
              <span className="h-1.5 w-1.5 rounded-full bg-current" />
              {listening.error ? "错误" : listening.isListening ? "正在识别" : "未识别"}
            </span>
          </div>

          <dl className="rounded-md border border-panel-border bg-app-background/60">
            <div className="flex items-center justify-between gap-3 border-b border-divider px-3.5 py-2">
              <dt className="text-sm text-text-primary">当前模型</dt>
              <dd className="truncate text-sm text-text-secondary">{modelName ?? "未知模型"}</dd>
            </div>
            <div className="flex items-center justify-between gap-3 px-3.5 py-2">
              <dt className="text-sm text-text-primary">麦克风</dt>
              <dd className="truncate text-sm text-text-secondary">{device || "默认设备"}</dd>
            </div>
          </dl>

          <div className="rounded-md border border-panel-border bg-app-background/60">
            <div className="border-b border-divider px-3.5 py-2">
              <p className="text-sm font-medium text-text-primary">实时转写结果</p>
            </div>
            {results.partial ? (
              <p className="px-3.5 py-3 text-sm text-text-muted">{results.partial}</p>
            ) : (
              <p className="px-3.5 py-3 text-sm text-text-muted">
                {listening.isListening ? "聆听中…" : "尚未开始识别"}
              </p>
            )}
            {results.segments.length > 0 && (
              <ul className="max-h-56 overflow-y-auto border-t border-divider px-3.5">
                {results.segments.map((s) => (
                  <li
                    key={s.id}
                    className="flex items-start justify-between gap-3 border-b border-divider py-1.5 text-sm last:border-b-0"
                  >
                    <span className="min-w-0 flex-1 text-text-primary">{s.text}</span>
                    <span className="shrink-0 text-xs text-text-muted">{s.at}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>

          {listening.error && (
            <Alert variant="destructive">
              <CircleAlert className="h-4 w-4" />
              <AlertDescription className="whitespace-pre-wrap">{listening.error}</AlertDescription>
            </Alert>
          )}
        </div>

        <div className="border-t border-divider px-5 py-3">
          <p className="text-xs text-text-muted">在本窗口内开启的识别，关闭时自动停止。</p>
        </div>
      </div>
    </div>
  );
}
