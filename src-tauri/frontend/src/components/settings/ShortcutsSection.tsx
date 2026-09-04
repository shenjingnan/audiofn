import { Keyboard } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { useToast } from "@/components/ui/toast";
import { api } from "@/lib/tauri";
import type { ShortcutActionId } from "@/types/tauri";
import { acceleratorFromEvent, formatAccelerator } from "./accelerator";

/** 可绑定操作清单（id 与 Rust `ShortcutAction::as_str` 一致）。 */
const ACTIONS: { id: ShortcutActionId; label: string; hint: string }[] = [
  { id: "open_settings", label: "打开设置", hint: "随时打开设置窗口" },
];

const isMac = /mac/i.test(navigator.platform || navigator.userAgent);

/**
 * 设置页「快捷键」区块：为高频操作自定义系统级全局快捷键。
 *
 * 录制态只接受「修饰键 + 字母/数字/常见标点」组合（裸键忽略，Esc 取消）；
 * 保存走后端 set_shortcut（先注册成功再落盘，失败时原绑定保持）。
 */
export function ShortcutsSection() {
  const toast = useToast();
  const [bindings, setBindings] = useState<Record<string, string>>({});
  const [recording, setRecording] = useState<ShortcutActionId | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void api
      .getShortcuts()
      .then((b) => setBindings(b ?? {}))
      .catch(() => {});
  }, []);

  const submit = useCallback(
    async (action: ShortcutActionId, accelerator: string) => {
      setRecording(null);
      // 本地冲突防线：同键已绑定其他操作直接拦截（后端还有兜底校验）
      const conflict = Object.entries(bindings).find(
        ([a, acc]) => a !== action && acc === accelerator,
      );
      if (conflict) {
        const label = ACTIONS.find((x) => x.id === conflict[0])?.label ?? conflict[0];
        setError(`该快捷键已绑定到「${label}」`);
        return;
      }
      try {
        await api.setShortcut({ action, accelerator });
        setBindings((prev) => ({ ...prev, [action]: accelerator }));
        setError(null);
        toast.success("快捷键已保存");
      } catch (e) {
        setError(String(e));
      }
    },
    [bindings, toast],
  );

  // 录制态：捕获 window keydown（capture 阶段，避免输入框等抢先处理）
  useEffect(() => {
    if (!recording) return;
    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setRecording(null);
        setError(null);
        return;
      }
      const accelerator = acceleratorFromEvent(e);
      if (!accelerator) return; // 裸键/不支持的主键：忽略，等待有效组合
      void submit(recording, accelerator);
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [recording, submit]);

  const clear = async (action: ShortcutActionId) => {
    try {
      await api.clearShortcut({ action });
      setBindings((prev) => {
        const next = { ...prev };
        delete next[action];
        return next;
      });
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <section className="overflow-hidden rounded-[16px] border border-panel-border bg-panel-background">
      <div className="px-3.5 py-2.5">
        <div className="flex items-center gap-2.5">
          <Keyboard className="h-4 w-4 shrink-0 text-text-secondary" />
          <div>
            <h2 className="text-base font-semibold text-text-primary">快捷键</h2>
            <p className="mt-0.5 text-xs text-text-muted">
              为高频操作设置系统级全局快捷键（任意应用中可触发）；需包含修饰键
            </p>
          </div>
        </div>
      </div>

      <dl className="divide-y divide-divider">
        {ACTIONS.map(({ id, label, hint }) => {
          const bound = bindings[id];
          const isRecording = recording === id;
          return (
            <div key={id} className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
              <div className="min-w-0">
                <dt className="text-sm text-text-primary">{label}</dt>
                <dd className="mt-0.5 text-xs text-text-muted">{hint}</dd>
              </div>
              <div className="flex shrink-0 items-center gap-1.5">
                <Button
                  size="sm"
                  variant={bound ? "outline" : "ghost"}
                  aria-label={`设置 ${label} 快捷键`}
                  className={bound ? undefined : "text-text-muted"}
                  onClick={() => {
                    setError(null);
                    setRecording(isRecording ? null : id);
                  }}
                >
                  {isRecording
                    ? "按下组合键…（Esc 取消）"
                    : bound
                      ? formatAccelerator(bound, isMac)
                      : "未设置"}
                </Button>
                {bound && (
                  <Button
                    size="sm"
                    variant="ghost"
                    aria-label={`清除 ${label} 快捷键`}
                    onClick={() => void clear(id)}
                  >
                    清除
                  </Button>
                )}
              </div>
            </div>
          );
        })}
      </dl>

      {error && <div className="px-3.5 pb-2.5 text-xs text-destructive">{error}</div>}
    </section>
  );
}
