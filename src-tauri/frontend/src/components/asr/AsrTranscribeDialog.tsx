import { Check, CircleAlert, Copy, FileAudio, Loader2, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useToast } from "@/components/ui/toast";
import { useAsrTranscribe } from "@/hooks/useAsrTranscribe";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";
import { asrModelKindLabel, modelNameFromDir } from "./asrMeta";

const EXIT_MS = 200;

interface AsrTranscribeDialogProps {
  open: boolean;
  onClose: () => void;
  /** 打开即自动转写模型自带 test_wavs 示例音频（离线模型「测试识别」） */
  autoRun?: boolean;
}

/**
 * 转写文件对话框：选择 wav → 后端整段转写（SenseVoice / Whisper / zipformer 均可用）。
 * 离线模型（无实时识别）的主入口；结果含复制按钮。
 */
export function AsrTranscribeDialog({ open, onClose, autoRun }: AsrTranscribeDialogProps) {
  const { asr } = useRuntime();
  const toast = useToast();
  const { pickAndTranscribe, runDefaultTest, transcribing, error, result, clear } =
    useAsrTranscribe();
  const [mounted, setMounted] = useState(open);
  const [closing, setClosing] = useState(false);
  const [copied, setCopied] = useState(false);
  const autoRunHandled = useRef(false);

  useEffect(() => {
    if (open) {
      setMounted(true);
      setClosing(false);
      autoRunHandled.current = false;
    }
  }, [open]);

  // 离线「测试识别」：打开即自动转写模型自带示例音频（仅触发一次）
  // biome-ignore lint/correctness/useExhaustiveDependencies: 有意仅随「打开且模型就绪」触发一次，避免回调标识变化重复转写
  useEffect(() => {
    if (open && mounted && autoRun && !autoRunHandled.current) {
      autoRunHandled.current = true;
      void runDefaultTest();
    }
  }, [open, mounted, autoRun]);

  const finishClose = useCallback(() => {
    setMounted(false);
    setClosing(false);
    onClose();
  }, [onClose]);

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

  // 关闭后清空上一次结果（下次打开重新选择）
  // biome-ignore lint/correctness/useExhaustiveDependencies: 有意仅在关闭时清空，clear/mounted 标识变化不应额外触发
  useEffect(() => {
    if (!open && mounted) clear();
  }, [open]);

  if (!mounted) return null;

  const modelName = modelNameFromDir(asr.config.config?.model_dir);
  const kind = asr.config.config?.model_type ?? "zipformer";

  const handleCopy = async () => {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      toast.error("复制失败");
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-label="转写音频文件"
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
          <h3 className="text-sm font-semibold text-text-primary">转写音频文件</h3>
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
          <dl className="rounded-md border border-panel-border bg-app-background/60">
            <div className="flex items-center justify-between gap-3 border-b border-divider px-3.5 py-2">
              <dt className="text-sm text-text-primary">当前模型</dt>
              <dd className="truncate text-sm text-text-secondary">
                {modelName ? `${modelName} · ${asrModelKindLabel(kind)}` : "未知模型"}
              </dd>
            </div>
          </dl>

          <Button
            className="w-full"
            disabled={transcribing}
            onClick={() => void pickAndTranscribe()}
          >
            {transcribing ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <FileAudio className="h-4 w-4" />
            )}
            {transcribing ? "转写中…" : result ? "重新选择音频…" : "选择音频文件…"}
          </Button>

          {transcribing && (
            <p className="text-center text-xs text-text-muted">整段转写中，请稍候…</p>
          )}

          {error && (
            <Alert variant="destructive">
              <CircleAlert className="h-4 w-4" />
              <AlertDescription className="whitespace-pre-wrap">{error}</AlertDescription>
            </Alert>
          )}

          {result && (
            <div className="rounded-md border border-panel-border bg-app-background/60">
              <div className="flex items-center justify-between border-b border-divider px-3.5 py-2">
                <p className="text-sm font-medium text-text-primary">转写结果</p>
                <Button variant="ghost" size="sm" className="h-7 gap-1.5" onClick={handleCopy}>
                  {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
                  {copied ? "已复制" : "复制"}
                </Button>
              </div>
              <p className="whitespace-pre-wrap px-3.5 py-3 text-sm text-text-primary">
                {result.text}
              </p>
            </div>
          )}
        </div>

        <div className="border-t border-divider px-5 py-3">
          <p className="text-xs text-text-muted">转写使用当前设为「当前」的识别模型。</p>
        </div>
      </div>
    </div>
  );
}
