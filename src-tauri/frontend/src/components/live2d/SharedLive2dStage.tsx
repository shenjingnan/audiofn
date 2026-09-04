import type { Ref } from "react";
import { useEffect, useImperativeHandle, useRef } from "react";
import type { ClaimHandle, Live2dCatalog, PreviewSlotCallbacks } from "./previewManager";
import { getPreviewManager } from "./previewManager";

/** SharedLive2dStage 的命令式句柄：动作/表情播放（卸载后调用安全 no-op）。 */
export type SharedLive2dStageHandle = {
  playMotion: (group: string, index: number) => Promise<boolean>;
  applyExpression: (index: number) => Promise<boolean>;
  resetExpression: () => void;
};

interface SharedLive2dStageProps {
  /** 模型清单文件的 asset:// URL，null 时清屏。 */
  modelUrl: string | null;
  width: number;
  height: number;
  className?: string;
  /** 模型在画布内的等比缩放（<1 缩小、>1 放大，默认 1 = 完整 contain 填充）。 */
  modelScale?: number;
  /** 强制重载信号：变化时销毁重建模型实例（等价 React key 重挂载，供概览页失败重试）。 */
  reloadKey?: string | number;
  /** 渲染初始化或模型加载失败时的回调。 */
  onError?: (error: Error) => void;
  /** 模型加载完成、可计算角色真实边界时回调（缓存命中也会触发）。 */
  onModelMetrics?: (metrics: { aspectRatio: number }) => void;
  /** 模型上舞台、画布可用时回调（缓存命中也会触发，上层需自行去重）。 */
  onModelReady?: (canvas: HTMLCanvasElement) => void;
  /** 模型动作/表情目录就绪回调（缓存命中也会触发，全量覆盖语义）。 */
  onModelCatalog?: (catalog: Live2dCatalog | null) => void;
  /** 命令式句柄（React 19 ref as prop）：播放动作/表情与重置。 */
  ref?: Ref<SharedLive2dStageHandle>;
}

/**
 * 共享 Live2D 预览组件（设置窗口内概览/伙伴页通用）：
 * 挂载时向 previewManager 单例 claim 舞台（canvas 移入本组件的 slot div），
 * 卸载时 release（canvas 移回离屏停放）。切页只是移动 canvas + resize，
 * 不再每次销毁重建 WebGL 上下文与模型（详见 previewManager.ts）。
 *
 * 桌宠窗口（独立 WebView）继续使用 Live2dStage。
 */
export function SharedLive2dStage({
  modelUrl,
  width,
  height,
  className,
  modelScale = 1,
  reloadKey = "default",
  onError,
  onModelMetrics,
  onModelReady,
  onModelCatalog,
  ref,
}: SharedLive2dStageProps) {
  const slotRef = useRef<HTMLDivElement>(null);
  const handleRef = useRef<ClaimHandle | null>(null);
  // 初始尺寸/缩放供 claim 使用（与 Live2dStage 相同的 ref 模式，避免 effect 依赖重占用）；
  // 后续变化由 updateLayout effect 同步给 Manager。
  const sizeRef = useRef({ width, height, modelScale });
  sizeRef.current = { width, height, modelScale };

  // 回调挂在稳定对象上、每次渲染更新字段（与 Live2dStage 的 ref 模式一致），
  // 保证 claim 不因回调身份变化重做，Manager 又总能读到最新闭包。
  const callbacksRef = useRef<PreviewSlotCallbacks>({});
  callbacksRef.current.onError = onError;
  callbacksRef.current.onModelMetrics = onModelMetrics;
  callbacksRef.current.onModelReady = onModelReady;
  callbacksRef.current.onModelCatalog = onModelCatalog;

  // 占用 / 释放共享舞台（仅随组件挂载/卸载，StrictMode 双挂载由 Manager 幂等处理）。
  useEffect(() => {
    const element = slotRef.current;
    if (!element) return;
    handleRef.current = getPreviewManager().claim({
      element,
      width: sizeRef.current.width,
      height: sizeRef.current.height,
      modelScale: sizeRef.current.modelScale,
      callbacks: callbacksRef.current,
    });
    return () => {
      handleRef.current?.release();
      handleRef.current = null;
    };
  }, []);

  // 命令式句柄：转发到当前 claim 的 handle；释放后（handleRef.current 为 null）安全 no-op。
  useImperativeHandle(
    ref,
    () => ({
      playMotion: (group, index) =>
        handleRef.current?.playMotion(group, index) ?? Promise.resolve(false),
      applyExpression: (index) =>
        handleRef.current?.applyExpression(index) ?? Promise.resolve(false),
      resetExpression: () => handleRef.current?.resetExpression(),
    }),
    [],
  );

  // 尺寸/缩放变化：只 resize 并重新布局，不重载模型。
  useEffect(() => {
    handleRef.current?.updateLayout(width, height, modelScale);
  }, [width, height, modelScale]);

  // 展示/切换/清空模型（不依赖尺寸）。
  useEffect(() => {
    handleRef.current?.showModel(modelUrl, reloadKey);
  }, [modelUrl, reloadKey]);

  return <div ref={slotRef} className={className} />;
}
