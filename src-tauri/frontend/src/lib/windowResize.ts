/**
 * Compute a window's target top-left so that resizing keeps its center fixed.
 *
 * Tauri's `setSize` grows the window from the top-left corner by default, which makes
 * the (always centered) Live2D character appear to scale from the top-left. Compensate
 * by shifting the top-left by half the size delta: `newX = x + (w - newW) / 2`.
 *
 * Inputs and output must share one unit space (callers pass physical pixels), so the
 * helper stays free of DPI math.
 */
export interface WindowRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Target top-left position for a center-anchored resize. */
export function centeredResizeTarget(
  current: WindowRect,
  targetWidth: number,
  targetHeight: number,
): { x: number; y: number } {
  return {
    x: Math.round(current.x + (current.width - targetWidth) / 2),
    y: Math.round(current.y + (current.height - targetHeight) / 2),
  };
}
