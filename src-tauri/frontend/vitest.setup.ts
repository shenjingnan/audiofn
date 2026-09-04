import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";

// 麦克风选择等偏好写入 localStorage；每个用例结束后清理，避免跨用例泄漏记忆值。
afterEach(() => {
  localStorage.clear();
});

// pixi-live2d-display/cubism4 在模块顶层检查 window.Live2DCubismCore，
// 测试环境不加载 index.html 里的 Cubism Core script，这里 mock 一个占位全局
// 以通过其模块加载检查（测试不实际渲染模型，不会触发真实 runtime 调用）。
if (typeof window !== "undefined") {
  (window as unknown as { Live2DCubismCore: unknown }).Live2DCubismCore ??= {};
}

// Radix Slider（`@radix-ui/react-use-size`）依赖 ResizeObserver，jsdom 未实现，
// 高级参数组件用到 Slider，这里补一个空实现。
if (typeof window !== "undefined" && typeof window.ResizeObserver === "undefined") {
  class ResizeObserverStub implements ResizeObserver {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  (window as unknown as { ResizeObserver: unknown }).ResizeObserver = ResizeObserverStub;
}

// Radix Select 在 pointer 交互时调用 hasPointerCapture / setPointerCapture /
// releasePointerCapture，jsdom 未实现这些 API，补空实现使下拉菜单可交互。
if (typeof Element !== "undefined" && typeof Element.prototype.hasPointerCapture !== "function") {
  Element.prototype.hasPointerCapture = () => false;
}
if (typeof Element !== "undefined" && typeof Element.prototype.setPointerCapture !== "function") {
  Element.prototype.setPointerCapture = () => {};
}
if (
  typeof Element !== "undefined" &&
  typeof Element.prototype.releasePointerCapture !== "function"
) {
  Element.prototype.releasePointerCapture = () => {};
}

// Radix Select 打开时对候选滚动定位，jsdom 未实现 scrollIntoView，补空实现。
if (typeof Element !== "undefined" && typeof Element.prototype.scrollIntoView !== "function") {
  Element.prototype.scrollIntoView = () => {};
}

// 模型列表无限滚动依赖 IntersectionObserver，jsdom 未实现，补空实现（测试不实际滚动）。
if (typeof window !== "undefined" && typeof window.IntersectionObserver === "undefined") {
  class IntersectionObserverStub {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
    takeRecords(): IntersectionObserverEntry[] {
      return [];
    }
    root = null;
    rootMargin = "";
    thresholds = [];
  }
  (window as unknown as { IntersectionObserver: unknown }).IntersectionObserver =
    IntersectionObserverStub as unknown as typeof IntersectionObserver;
}
