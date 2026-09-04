import { cleanup, render } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SharedLive2dStageHandle } from "./SharedLive2dStage";
import { SharedLive2dStage } from "./SharedLive2dStage";

/**
 * 只验证组件与 previewManager 的接线契约（claim/release/updateLayout/showModel 的
 * 时机与参数）；Manager 本身的行为在 previewManager.test.ts 覆盖。
 */
const { getManagerMock, claimMock, handleStub } = vi.hoisted(() => {
  const handleStub = {
    id: 1,
    updateLayout: vi.fn(),
    showModel: vi.fn(),
    release: vi.fn(),
    playMotion: vi.fn(async () => true),
    applyExpression: vi.fn(async () => true),
    resetExpression: vi.fn(),
  };
  // 参数类型对齐 ClaimOptions，使 mock.calls[0][0] 可直接断言。
  const claimMock = vi.fn(
    (_opts: {
      element: HTMLDivElement;
      width: number;
      height: number;
      modelScale: number;
      callbacks: Record<string, unknown>;
    }) => handleStub,
  );
  const getManagerMock = vi.fn(() => ({ claim: claimMock }));
  return { getManagerMock, claimMock, handleStub };
});

vi.mock("./previewManager", () => ({ getPreviewManager: getManagerMock }));

function baseProps() {
  return {
    modelUrl: "asset://a.model3.json" as string | null,
    width: 300,
    height: 200,
    modelScale: 0.8,
    className: "h-full w-full",
  };
}

beforeEach(() => {
  claimMock.mockClear();
  handleStub.updateLayout.mockClear();
  handleStub.showModel.mockClear();
  handleStub.release.mockClear();
  handleStub.playMotion.mockClear();
  handleStub.applyExpression.mockClear();
  handleStub.resetExpression.mockClear();
});

afterEach(cleanup);

describe("SharedLive2dStage", () => {
  it("挂载即 claim：传入 slot 元素、尺寸缩放与回调对象，并先 claim 后 showModel", () => {
    render(<SharedLive2dStage {...baseProps()} />);

    expect(claimMock).toHaveBeenCalledTimes(1);
    const opts = claimMock.mock.calls[0][0];
    expect(opts.element).toBeInstanceOf(HTMLDivElement);
    expect(opts.width).toBe(300);
    expect(opts.height).toBe(200);
    expect(opts.modelScale).toBe(0.8);
    expect(opts.callbacks).toHaveProperty("onError");
    expect(opts.callbacks).toHaveProperty("onModelMetrics");
    expect(opts.callbacks).toHaveProperty("onModelReady");

    // 挂载时序：claim（占用舞台）必须先于 showModel（展示模型）。
    expect(claimMock.mock.invocationCallOrder[0]).toBeLessThan(
      handleStub.showModel.mock.invocationCallOrder[0],
    );
    expect(handleStub.showModel).toHaveBeenCalledWith("asset://a.model3.json", "default");
  });

  it("卸载时 release 占用", () => {
    const { unmount } = render(<SharedLive2dStage {...baseProps()} />);
    unmount();
    expect(handleStub.release).toHaveBeenCalledTimes(1);
  });

  it("尺寸/缩放变化只走 updateLayout，不重新 claim", () => {
    const { rerender } = render(<SharedLive2dStage {...baseProps()} />);
    rerender(<SharedLive2dStage {...baseProps()} width={500} height={400} modelScale={1} />);

    expect(claimMock).toHaveBeenCalledTimes(1);
    expect(handleStub.updateLayout).toHaveBeenCalledWith(500, 400, 1);
  });

  it("modelUrl / reloadKey 变化走 showModel；不传 reloadKey 时用稳定默认值", () => {
    const { rerender } = render(<SharedLive2dStage {...baseProps()} reloadKey="k1" />);
    expect(handleStub.showModel).toHaveBeenCalledWith("asset://a.model3.json", "k1");

    rerender(
      <SharedLive2dStage {...baseProps()} modelUrl="asset://b.model3.json" reloadKey="k1" />,
    );
    expect(handleStub.showModel).toHaveBeenLastCalledWith("asset://b.model3.json", "k1");

    rerender(
      <SharedLive2dStage {...baseProps()} modelUrl="asset://b.model3.json" reloadKey="k2" />,
    );
    expect(handleStub.showModel).toHaveBeenLastCalledWith("asset://b.model3.json", "k2");

    // modelUrl 与 reloadKey 都不变的重渲染不重复 showModel（3 次 = 初始 + 2 次变化）。
    rerender(
      <SharedLive2dStage {...baseProps()} modelUrl="asset://b.model3.json" reloadKey="k2" />,
    );
    expect(handleStub.showModel).toHaveBeenCalledTimes(3);
  });

  it("StrictMode 双挂载：claim→release→claim，最终仍占用", () => {
    render(
      <StrictMode>
        <SharedLive2dStage {...baseProps()} />
      </StrictMode>,
    );

    expect(claimMock).toHaveBeenCalledTimes(2);
    expect(handleStub.release).toHaveBeenCalledTimes(1);
  });

  it("透传 onModelCatalog 回调，并通过 ref 暴露播放/重置方法", async () => {
    const ref = { current: null as SharedLive2dStageHandle | null };
    const onModelCatalog = vi.fn();
    render(<SharedLive2dStage {...baseProps()} ref={ref} onModelCatalog={onModelCatalog} />);

    const opts = claimMock.mock.calls[0][0];
    expect(opts.callbacks).toHaveProperty("onModelCatalog");

    const handle = ref.current;
    if (!handle) throw new Error("SharedLive2dStage handle 未挂载");
    expect(await handle.playMotion("Extra", 0)).toBe(true);
    expect(handleStub.playMotion).toHaveBeenCalledWith("Extra", 0);
    expect(await handle.applyExpression(1)).toBe(true);
    expect(handleStub.applyExpression).toHaveBeenCalledWith(1);
    handle.resetExpression();
    expect(handleStub.resetExpression).toHaveBeenCalledTimes(1);
  });
});
