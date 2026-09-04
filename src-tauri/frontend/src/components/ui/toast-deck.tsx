import { X } from "lucide-react";
import { useEffect, useState } from "react";
import { Toast as ToastPrimitive } from "@base-ui/react/toast";

import { cn } from "@/lib/utils";

/**
 * 桌宠窗口专用 toast（基于 shadcn/ui base-nova 变体，见
 * https://ui.shadcn.com/docs/components/base/toast；卡片视觉为原版样式）。
 *
 * iOS 风格 stacked 堆叠：最新卡片完整显示在最前，旧卡叠在其后，每层从顶部
 * 露出 10px 并轻微缩放/减淡/降 z-index；最多 3 个可见层（更多由 Provider
 * limit 标记 data-limited 隐藏，留在队列，前面消失后自动浮现）。所有卡片
 * absolute 锚定同一位置，不随数量纵向展开。
 *
 * 布局与动画自持（layer/entered/ending 三态合成内联 transform），不依赖
 * Base UI 的 `--toast-offset-y` 体系——后者在 nonactivating panel 下高度测量
 * 失灵（真机实证）。队列/limit/hover 暂停/过期由 Base UI + EventBubble
 * 自持时钟负责。
 *
 * 其余桌宠适配：viewport 锚窗口顶部居中；仅最前层可交互；Root 拦截
 * mousedown/contextmenu 冒泡（避免在卡片上按下时拖走窗口）。
 *
 * 命名为 toast-deck 以避开本目录的自研 toast.tsx（设置窗口通知，10+ 处引用）。
 */

export const createToastManager = ToastPrimitive.createToastManager;

/** 每层向上露出的高度（px）。 */
const LAYER_PEEK_PX = 10;
/** 每层缩放步长（layer 1 → 0.98，layer 2 → 0.96）。 */
const LAYER_SCALE_STEP = 0.02;
/** 各层透明度（递减营造纵深）。 */
const LAYER_OPACITY = [1, 0.92, 0.85];
/** 入场动画起点：自下方 14px 淡入。 */
const ENTER_OFFSET_PX = 14;
/** 退场动画：向右 24px 淡出。 */
const EXIT_OFFSET_PX = 24;

function ToastPortal({ ...props }: ToastPrimitive.Portal.Props) {
  return <ToastPrimitive.Portal data-slot="toast-portal" {...props} />;
}

function ToastViewport({ className, ...props }: ToastPrimitive.Viewport.Props) {
  return (
    <ToastPrimitive.Viewport
      data-slot="toast-viewport"
      className={cn(
        "pointer-events-none fixed inset-x-0 top-0 z-10 mx-auto h-[72px] w-full max-w-[340px] outline-none",
        className,
      )}
      {...props}
    />
  );
}

/**
 * 单张卡片：`layer`（0 = 最新最前）决定 translateY/scale/opacity/z-index 与
 * 可交互性；`entered`（挂载下一帧）驱动从下方淡入；`ending`（Base UI 退出
 * 状态）驱动向右淡出。三者合成单一内联 transform，由 CSS transition 平滑
 * 补间（新卡入队推挤旧卡、前面消失后补位，均无跳变）。
 */
function Toast({
  layer,
  className,
  onMouseDown,
  onContextMenu,
  ...props
}: ToastPrimitive.Root.Props & { layer: number }) {
  const ending = props.toast?.transitionStatus === "ending";
  const [entered, setEntered] = useState(false);
  useEffect(() => {
    // 双 rAF：确保浏览器先把初始（透明+下移）样式绘制一帧，再切换就位态触发过渡。
    let r2 = 0;
    const r1 = requestAnimationFrame(() => {
      r2 = requestAnimationFrame(() => setEntered(true));
    });
    return () => {
      cancelAnimationFrame(r1);
      cancelAnimationFrame(r2);
    };
  }, []);

  const peekY = -layer * LAYER_PEEK_PX;
  const scale = 1 - layer * LAYER_SCALE_STEP;
  const enterY = entered ? 0 : ENTER_OFFSET_PX;
  const exitX = ending ? EXIT_OFFSET_PX : 0;
  const opacity = ending ? 0 : entered ? (LAYER_OPACITY[layer] ?? 0.8) : 0;

  return (
    <ToastPrimitive.Root
      data-slot="toast"
      className={cn(
        "cn-toast group/toast pointer-events-none absolute bottom-0 left-1/2 w-full origin-bottom rounded-lg border bg-popover text-popover-foreground outline-none select-none",
        "transition-[transform,opacity] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]",
        "data-limited:hidden",
        layer === 0 ? "shadow-lg" : "shadow-md",
        className,
      )}
      style={{
        transform: `translateX(calc(-50% + ${exitX}px)) translateY(${peekY + enterY}px) scale(${scale})`,
        opacity,
        zIndex: 10 - layer,
        pointerEvents: layer === 0 && !ending ? "auto" : "none",
      }}
      onMouseDown={(e) => {
        onMouseDown?.(e);
        e.stopPropagation();
      }}
      onContextMenu={(e) => {
        onContextMenu?.(e);
        e.stopPropagation();
      }}
      {...props}
    />
  );
}

function ToastContent({ className, ...props }: ToastPrimitive.Content.Props) {
  return (
    <ToastPrimitive.Content
      data-slot="toast-content"
      className={cn("flex items-center gap-3 overflow-hidden p-3", className)}
      {...props}
    />
  );
}

function ToastTitle({ className, ...props }: ToastPrimitive.Title.Props) {
  return (
    <ToastPrimitive.Title
      data-slot="toast-title"
      className={cn("min-w-0 flex-1 text-sm font-medium leading-snug", className)}
      {...props}
    />
  );
}

function ToastClose({ className, ...props }: ToastPrimitive.Close.Props) {
  return (
    <ToastPrimitive.Close
      data-slot="toast-close"
      aria-label="关闭"
      className={cn(
        "relative shrink-0 rounded-md p-1 text-muted-foreground transition-colors hover:text-foreground",
        className,
      )}
      {...props}
    >
      <X aria-hidden="true" className="size-3.5" />
    </ToastPrimitive.Close>
  );
}

function ToastList() {
  const { toasts } = ToastPrimitive.useToastManager();

  // toasts 最新在前：layer = 数组下标（0 最前），第 4 起被 limit 标记 data-limited。
  return toasts.map((toastItem, layer) => (
    <Toast key={toastItem.id} toast={toastItem} layer={layer}>
      <ToastContent>
        <ToastTitle />
        <ToastClose />
      </ToastContent>
    </Toast>
  ));
}

/** 挂载一次即可：管理器私有于调用方（toastManager prop），卡片入队走 manager.add。 */
function Toaster({ children, ...props }: ToastPrimitive.Provider.Props) {
  return (
    <ToastPrimitive.Provider {...props}>
      {children}
      <ToastPortal>
        <ToastViewport>
          <ToastList />
        </ToastViewport>
      </ToastPortal>
    </ToastPrimitive.Provider>
  );
}

export {
  Toast,
  ToastClose,
  ToastContent,
  ToastPortal,
  ToastTitle,
  ToastViewport,
  Toaster,
};
