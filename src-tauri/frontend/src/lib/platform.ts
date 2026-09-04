/** 平台判定：基于 WebView userAgent 子串匹配（沿用项目既有判断方式）。
 *
 * 测试可通过 Object.defineProperty(navigator, "userAgent") 覆盖后渲染。
 */

/** macOS（系统红绿灯在左上角，窗口圆角由系统绘制）。 */
export function isMacOs(): boolean {
  return navigator.userAgent.includes("Macintosh");
}

/** Windows（自绘三键悬浮右上角，不透明方角窗口）。 */
export function isWindows(): boolean {
  return navigator.userAgent.includes("Windows");
}
