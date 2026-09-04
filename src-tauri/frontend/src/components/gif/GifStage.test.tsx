import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { GifStage } from "./GifStage";

describe("GifStage", () => {
  it("url 为 null 时不渲染 img", () => {
    const { container } = render(<GifStage url={null} width={400} height={480} />);
    expect(container.querySelector("img")).toBeNull();
  });

  it("渲染 img 并透传 url", () => {
    render(<GifStage url="asset://localhost/x.gif" width={400} height={480} />);
    const img = screen.getByRole("img");
    expect(img).toHaveAttribute("src", "asset://localhost/x.gif");
  });

  it("加载成功后上报真实宽高比", () => {
    const onModelMetrics = vi.fn();
    render(
      <GifStage
        url="asset://localhost/x.gif"
        width={400}
        height={480}
        onModelMetrics={onModelMetrics}
      />,
    );
    const img = screen.getByRole("img");
    Object.defineProperty(img, "naturalWidth", { value: 850 });
    Object.defineProperty(img, "naturalHeight", { value: 1000 });
    fireEvent.load(img);
    expect(onModelMetrics).toHaveBeenCalledWith({ aspectRatio: 0.85 });
  });

  it("natural 尺寸缺失（0）不上报", () => {
    const onModelMetrics = vi.fn();
    render(
      <GifStage
        url="asset://localhost/x.gif"
        width={400}
        height={480}
        onModelMetrics={onModelMetrics}
      />,
    );
    fireEvent.load(screen.getByRole("img"));
    expect(onModelMetrics).not.toHaveBeenCalled();
  });

  it("加载失败上报错误", () => {
    const onError = vi.fn();
    render(
      <GifStage url="asset://localhost/bad.gif" width={400} height={480} onError={onError} />,
    );
    fireEvent.error(screen.getByRole("img"));
    expect(onError).toHaveBeenCalled();
  });
});
