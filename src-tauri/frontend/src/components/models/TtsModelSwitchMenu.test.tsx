import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TtsModelSwitchMenu } from "./TtsModelSwitchMenu";

// stub 选择合成模型弹窗：只记录 props，避免其内部（toast/invoke）整链依赖。
const { dialogProps } = vi.hoisted(() => ({
  dialogProps: { last: null as { open: boolean; onClose: () => void } | null },
}));

vi.mock("@/components/tts/TtsModelDialog", () => ({
  TtsModelDialog: (props: { open: boolean; onClose: () => void }) => {
    dialogProps.last = props;
    return props.open ? <div data-testid="tts-dialog">选择合成模型弹窗</div> : null;
  },
}));

// mock runtime：tts 切片可变（model_dir）。注意 TtsState 的 config 是单层（非 asr 的嵌套两层）。
const { state, navProbe } = vi.hoisted(() => ({
  state: {
    tts: null as { config: { model_dir: string; models_present: boolean } | null } | null,
  },
  // 模拟浏览器原生行为：a 内嵌 button 点击的默认动作是跟随祖先 href；
  // jsdom 不实现该行为，这里以 defaultPrevented 为准计数「原生导航」次数。
  navProbe: { count: 0 },
}));

vi.mock("@/providers/RuntimeContext", () => ({
  useRuntime: () => ({ tts: state.tts }),
}));

function makeTtsConfig(modelDir?: string) {
  state.tts = {
    config: {
      model_dir:
        modelDir ?? "/home/user/.audiofn/models/sherpa-onnx-zipvoice-distill-int8-zh-en-emilia",
      models_present: true,
    },
  };
}

/** 挂真实链接行验证「不触发导航」；location 探针放在 /models。 */
function Probe() {
  const location = useLocation();
  return (
    <>
      <div data-testid="location">{location.pathname}</div>
      <a
        href="/models/tts"
        data-testid="row-link"
        onClick={(e) => {
          // 模拟原生「激活祖先 a」：拦截层调用了 preventDefault 则视为已阻止导航。
          if (!e.defaultPrevented) navProbe.count++;
        }}
      >
        <TtsModelSwitchMenu />
      </a>
    </>
  );
}

function renderMenu() {
  return render(
    <MemoryRouter initialEntries={["/models"]}>
      <Routes>
        <Route path="/models" element={<Probe />} />
        <Route path="/models/tts" element={<div>配置页</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  dialogProps.last = null;
  navProbe.count = 0;
  makeTtsConfig();
});

describe("TtsModelSwitchMenu 模型快速切换（弹窗版）", () => {
  it("模型名文本 + 「选择模型」按钮", () => {
    renderMenu();
    expect(screen.getByText("sherpa-onnx-zipvoice-distill-int8-zh-en-emilia")).toBeInTheDocument();
    const button = screen.getByRole("button", { name: "选择合成模型" });
    expect(button).toHaveTextContent("选择模型");
  });

  it("点击切换按钮打开选择合成模型弹窗", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(screen.getByRole("button", { name: "选择合成模型" }));

    expect(dialogProps.last?.open).toBe(true);
    expect(screen.getByTestId("tts-dialog")).toBeInTheDocument();
  });

  it("弹窗 onClose 回调关闭后可再次打开", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(screen.getByRole("button", { name: "选择合成模型" }));
    expect(dialogProps.last?.open).toBe(true);

    // onClose 触发 setState，等待 stub 以 open=false 重渲染。
    act(() => dialogProps.last?.onClose());
    await waitFor(() => expect(dialogProps.last?.open).toBe(false));

    await user.click(screen.getByRole("button", { name: "选择合成模型" }));
    await waitFor(() => expect(dialogProps.last?.open).toBe(true));
  });

  it("点击行内按钮/弹窗不触发所在行的链接导航（含原生 href 默认行为）", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(screen.getByRole("button", { name: "选择合成模型" }));
    expect(screen.getByTestId("tts-dialog")).toBeInTheDocument();
    expect(screen.getByTestId("location")).toHaveTextContent("/models");
    // 回归：拦截层必须 preventDefault，否则浏览器原生跟随 <a href> 整页跳转。
    expect(navProbe.count).toBe(0);
  });
});
