import { AlertCircle, CheckCircle2, Info, X } from "lucide-react";
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
} from "react";
import { cn } from "@/lib/utils";

type ToastVariant = "default" | "success" | "error" | "warning";

interface ToastItem {
  id: number;
  message: string;
  variant: ToastVariant;
}

interface ToastApi {
  toast: (message: string, variant?: ToastVariant) => void;
  success: (message: string) => void;
  error: (message: string) => void;
  warning: (message: string) => void;
  dismiss: (id: number) => void;
}

const ToastContext = createContext<ToastApi | null>(null);

/** 使用轻量 Toast（必须在 ToastProvider 内）。 */
export function useToast(): ToastApi {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    throw new Error("useToast 必须在 ToastProvider 内使用");
  }
  return ctx;
}

const ICONS: Record<ToastVariant, { icon: typeof Info; className: string }> = {
  default: { icon: Info, className: "text-blue-600" },
  success: { icon: CheckCircle2, className: "text-emerald-600" },
  error: { icon: AlertCircle, className: "text-red-600" },
  warning: { icon: AlertCircle, className: "text-amber-600" },
};

const DURATION_MS = 3500;

/** 轻量 ToastProvider：右上角 viewport，自动消失，复用 AudioFn panel 视觉。 */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const nextId = useRef(1);

  const dismiss = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const toast = useCallback(
    (message: string, variant: ToastVariant = "default") => {
      const id = nextId.current++;
      setToasts((prev) => [...prev, { id, message, variant }]);
      window.setTimeout(() => dismiss(id), DURATION_MS);
    },
    [dismiss],
  );

  const api = useMemo<ToastApi>(
    () => ({
      toast,
      success: (m) => toast(m, "success"),
      error: (m) => toast(m, "error"),
      warning: (m) => toast(m, "warning"),
      dismiss,
    }),
    [toast, dismiss],
  );

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div className="pointer-events-none fixed right-4 top-4 z-[100] flex w-80 flex-col gap-2">
        {toasts.map((t) => {
          const cfg = ICONS[t.variant];
          const Icon = cfg.icon;
          return (
            <div
              key={t.id}
              role="status"
              className="pointer-events-auto flex items-start gap-3 rounded-[12px] border border-panel-border bg-panel-background px-3.5 py-3 shadow-[0_1px_2px_rgba(15,23,42,0.05),0_8px_24px_rgba(15,23,42,0.1)] animate-in fade-in slide-in-from-top-2"
            >
              <Icon className={cn("mt-0.5 h-4 w-4 shrink-0", cfg.className)} />
              <p className="min-w-0 flex-1 text-sm leading-snug text-text-primary">{t.message}</p>
              <button
                type="button"
                aria-label="关闭"
                onClick={() => dismiss(t.id)}
                className="shrink-0 text-text-muted transition-colors hover:text-text-primary"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
          );
        })}
      </div>
    </ToastContext.Provider>
  );
}
