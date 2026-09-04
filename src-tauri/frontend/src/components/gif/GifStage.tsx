import type { CSSProperties } from "react";

interface GifStageProps {
  /** GIF 文件的 asset:// URL，null 时不渲染。 */
  url: string | null;
  width: number;
  height: number;
  className?: string;
  style?: CSSProperties;
  /** 渲染失败时的回调。 */
  onError?: (error: Error) => void;
  /** 加载完成、可读取真实宽高时回调（供上层自适应窗口尺寸，语义对齐 Live2dStage）。 */
  onModelMetrics?: (metrics: { aspectRatio: number }) => void;
}

/**
 * GIF 伙伴渲染组件：原生 `<img>` 播放（WebView 内建 GIF 解码，自动循环），
 * 不引入 PIXI——大 GIF 逐帧解码为纹理集内存不可行（见 GIF_COMPANION_DESIGN §4 D5）。
 *
 * 交互（拖拽/右键/滚轮缩放）由 CompanionRoot 的 DOM 层承担，这里 img 不接事件。
 * 切换伙伴换 url 后 `onLoad` 会再次触发，宽高比随之上报。
 */
export function GifStage({
  url,
  width,
  height,
  className,
  style,
  onError,
  onModelMetrics,
}: GifStageProps) {
  return (
    <div className={className} style={{ width, height, ...style }} data-testid="gif-stage">
      {url && (
        <img
          src={url}
          alt="伙伴"
          draggable={false}
          className="pointer-events-none h-full w-full select-none object-contain"
          onLoad={(e) => {
            const img = e.currentTarget;
            if (img.naturalWidth > 0 && img.naturalHeight > 0) {
              onModelMetrics?.({ aspectRatio: img.naturalWidth / img.naturalHeight });
            }
          }}
          onError={() => onError?.(new Error(`GIF 加载失败: ${url}`))}
        />
      )}
    </div>
  );
}
