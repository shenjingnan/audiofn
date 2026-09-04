import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { PropsLayer } from "./PropsLayer";

const LAYOUT = { x: 10, y: 20, scale: 2, canvasWidth: 300, canvasHeight: 200 };

describe("PropsLayer", () => {
  it("无布局时不渲染（模型未加载）", () => {
    const { container } = render(
      <PropsLayer layout={null} backgroundUrl={null} pressedKeys={[]} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("盒子几何来自画布→屏幕映射", () => {
    const { container } = render(
      <PropsLayer layout={LAYOUT} backgroundUrl={null} pressedKeys={[]} />,
    );
    const box = container.firstChild as HTMLElement;
    expect(box.style.left).toBe("10px");
    expect(box.style.top).toBe("20px");
    expect(box.style.width).toBe("600px"); // 300 * 2
    expect(box.style.height).toBe("400px"); // 200 * 2
  });

  it("渲染键盘背景图", () => {
    const { container } = render(
      <PropsLayer layout={LAYOUT} backgroundUrl="asset://background.png" pressedKeys={[]} />,
    );
    const imgs = container.querySelectorAll("img");
    expect(imgs).toHaveLength(1);
    expect(imgs[0].getAttribute("src")).toBe("asset://background.png");
  });

  it("按 pressedKeys 渲染爪子贴图（叠加在背景上）", () => {
    const { container } = render(
      <PropsLayer
        layout={LAYOUT}
        backgroundUrl="asset://background.png"
        pressedKeys={[{ key: "KeyA", url: "asset://KeyA.png", hand: "left" }]}
      />,
    );
    const imgs = container.querySelectorAll("img");
    expect(imgs).toHaveLength(2); // 背景 + 按键
    const srcs = Array.from(imgs).map((i) => i.getAttribute("src"));
    expect(srcs).toContain("asset://KeyA.png");
  });

  it("按键贴图增删（按下与松开）", () => {
    const { container, rerender } = render(
      <PropsLayer layout={LAYOUT} backgroundUrl="asset://background.png" pressedKeys={[]} />,
    );
    expect(container.querySelectorAll("img")).toHaveLength(1);

    rerender(
      <PropsLayer
        layout={LAYOUT}
        backgroundUrl="asset://background.png"
        pressedKeys={[{ key: "KeyA", url: "asset://KeyA.png", hand: "left" }]}
      />,
    );
    expect(container.querySelectorAll("img")).toHaveLength(2);

    rerender(
      <PropsLayer layout={LAYOUT} backgroundUrl="asset://background.png" pressedKeys={[]} />,
    );
    expect(container.querySelectorAll("img")).toHaveLength(1);
  });
});
