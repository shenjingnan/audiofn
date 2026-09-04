import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { AudioLines, CircleAlert, Mic, Plus, Save, Trash2, Upload, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";

/** 退出动画时长，需与卡片/遮罩的 duration 一致。 */
const EXIT_MS = 200;
/** 在线录音可选时长（秒）。 */
const RECORD_SECONDS = [3, 5, 10] as const;

interface TtsVoicesDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * 管理音色对话框：列出已保存的自定义音色（可删除）+ 添加音色
 * （上传音频或在线录音 → 命名 → 自动转写参考文本 → 保存到音色库）。
 * 自定义音色保存在 `~/.audiofn/voices/`，重启后仍可选用。
 */
export function TtsVoicesDialog({ open, onClose }: TtsVoicesDialogProps) {
  const { tts, anyListening, device } = useRuntime();
  const [mounted, setMounted] = useState(open);
  const [closing, setClosing] = useState(false);
  const [adding, setAdding] = useState(false);
  const [wavPath, setWavPath] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [referenceText, setReferenceText] = useState("");
  const [transcribing, setTranscribing] = useState(false);
  const [recording, setRecording] = useState(false);
  const [recordSeconds, setRecordSeconds] = useState(5);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 打开时挂载并重置添加表单状态
  useEffect(() => {
    if (open) {
      setMounted(true);
      setClosing(false);
      setAdding(false);
      setWavPath(null);
      setName("");
      setReferenceText("");
      setError(null);
    }
  }, [open]);

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

  // Esc 取消
  useEffect(() => {
    if (!mounted || closing) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [mounted, closing, close]);

  if (!mounted) return null;

  const customVoices = tts.voices.filter((v) => v.custom);

  const pickWav = async () => {
    const path = await openDialog({
      multiple: false,
      title: "选择参考音频",
      filters: [{ name: "WAV", extensions: ["wav"] }],
    });
    if (typeof path === "string") {
      setWavPath(path);
      setReferenceText("");
      setError(null);
    }
  };

  const handleRecord = async () => {
    setRecording(true);
    setError(null);
    try {
      const path = await tts.recordVoice(recordSeconds, device || null);
      setWavPath(path);
      setReferenceText("");
    } catch (e) {
      setError(String(e));
    } finally {
      setRecording(false);
    }
  };

  const handleTranscribe = async () => {
    if (!wavPath) return;
    setTranscribing(true);
    setError(null);
    try {
      const text = await api.transcribeReferenceAudio({ wavPath });
      setReferenceText(text);
    } catch (e) {
      setError(String(e));
    } finally {
      setTranscribing(false);
    }
  };

  const handleSave = async () => {
    if (!wavPath) return;
    setSaving(true);
    setError(null);
    try {
      await tts.saveVoice({ name, sourceWavPath: wavPath, referenceText });
      // 保存成功回到列表
      setAdding(false);
      setWavPath(null);
      setName("");
      setReferenceText("");
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id: string) => {
    setError(null);
    try {
      await tts.deleteVoice(id);
    } catch (e) {
      setError(String(e));
    }
  };

  const canSave = wavPath !== null && name.trim() !== "" && referenceText.trim() !== "";

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-label="管理音色"
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
          <h3 className="text-sm font-semibold text-text-primary">管理音色</h3>
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
          {adding ? (
            <>
              {/* 来源：上传或录音 */}
              <div className="space-y-1.5">
                <p className="text-sm text-text-primary">来源</p>
                <div className="flex flex-wrap items-center gap-2">
                  <Button variant="outline" size="sm" onClick={pickWav}>
                    <Upload className="h-4 w-4" />
                    上传音频
                  </Button>
                  <Select
                    value={String(recordSeconds)}
                    onValueChange={(v) => setRecordSeconds(Number(v))}
                    disabled={recording}
                  >
                    <SelectTrigger className="w-24" aria-label="录音时长">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {RECORD_SECONDS.map((s) => (
                        <SelectItem key={s} value={String(s)}>
                          {s} 秒
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={handleRecord}
                    disabled={recording || anyListening}
                  >
                    <Mic className="h-4 w-4" />
                    {recording ? "录音中…" : "开始录音"}
                  </Button>
                </div>
                {anyListening && !recording && (
                  <p className="text-xs text-text-muted">语音识别正在运行，录音暂不可用。</p>
                )}
                {wavPath && (
                  <p className="truncate font-mono text-xs text-text-muted" title={wavPath}>
                    {wavPath}
                  </p>
                )}
              </div>

              {/* 名称 */}
              <div className="space-y-1.5">
                <label className="text-sm text-text-primary" htmlFor="voice-name">
                  音色名称
                </label>
                <Input
                  id="voice-name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="例如：我的声音"
                />
              </div>

              {/* 参考文本 */}
              <div className="space-y-1.5">
                <div className="flex items-center justify-between gap-2">
                  <label className="text-sm text-text-primary" htmlFor="voice-ref">
                    参考文本
                  </label>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={handleTranscribe}
                    disabled={!wavPath || transcribing}
                  >
                    <AudioLines className="h-4 w-4" />
                    {transcribing ? "转写中…" : "自动转写"}
                  </Button>
                </div>
                <textarea
                  id="voice-ref"
                  className="w-full rounded-md border border-input bg-background p-2 text-sm outline-none focus:ring-1 focus:ring-ring"
                  rows={3}
                  value={referenceText}
                  onChange={(e) => setReferenceText(e.target.value)}
                  placeholder="参考音频的逐字转写文本（可点「自动转写」或手动填写，须与音频一致）"
                />
              </div>

              {error && (
                <Alert variant="destructive">
                  <CircleAlert className="h-4 w-4" />
                  <AlertDescription className="whitespace-pre-wrap">{error}</AlertDescription>
                </Alert>
              )}

              <div className="flex items-center justify-between gap-2 border-t border-divider pt-3">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    setError(null);
                    setAdding(false);
                  }}
                >
                  返回
                </Button>
                <Button size="sm" onClick={handleSave} disabled={!canSave || saving}>
                  <Save className="h-4 w-4" />
                  {saving ? "保存中…" : "保存音色"}
                </Button>
              </div>
            </>
          ) : (
            <>
              {customVoices.length === 0 ? (
                <p className="text-sm text-text-muted">
                  尚未保存自定义音色。点击下方「添加音色」上传或录音一个声音。
                </p>
              ) : (
                <ul className="divide-y divide-divider rounded-md border border-panel-border bg-app-background/60">
                  {customVoices.map((v) => (
                    <li
                      key={v.id}
                      className="flex items-center justify-between gap-3 px-3.5 py-2.5"
                    >
                      <div className="min-w-0">
                        <p className="text-sm font-medium text-text-primary">{v.name}</p>
                        <p className="truncate text-xs text-text-muted" title={v.reference_text}>
                          {v.reference_text}
                        </p>
                      </div>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 shrink-0 text-text-muted hover:bg-destructive hover:text-destructive-foreground"
                        onClick={() => handleDelete(v.id)}
                        aria-label={`删除音色 ${v.name}`}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </li>
                  ))}
                </ul>
              )}

              {error && (
                <Alert variant="destructive">
                  <CircleAlert className="h-4 w-4" />
                  <AlertDescription className="whitespace-pre-wrap">{error}</AlertDescription>
                </Alert>
              )}

              <div className="flex items-center justify-between gap-2 border-t border-divider pt-3">
                <p className="text-xs text-text-muted">自定义音色保存在本机音色库中。</p>
                <Button
                  size="sm"
                  onClick={() => {
                    setError(null);
                    setAdding(true);
                  }}
                >
                  <Plus className="h-4 w-4" />
                  添加音色
                </Button>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
