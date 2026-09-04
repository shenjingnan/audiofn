import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "@/components/ui/toast";
import type { RuntimeState } from "@/providers/RuntimeContext";
import { HomePage } from "./HomePage";

const { state } = vi.hoisted(() => ({
  // 可变 runtime 快照：单个用例按需替换 asr/tts 切片（贴近真实 RuntimeState）。
  state: { runtime: null as RuntimeState | null },
}));

// 概览页读取全局 runtime；直接 mock context 模块比挂载完整 App 轻得多。
vi.mock("@/providers/RuntimeContext", () => ({
  useRuntime: () => state.runtime,
}));

// ---- runtime 切片工厂：只填概览推导真正读取的字段 ----

function makeAsr(o?: {
  enabled?: boolean;
  modelsPresent?: boolean;
  isDictating?: boolean;
  pending?: boolean;
  error?: string | null;
}) {
  return {
    config: {
      config: { enabled: o?.enabled ?? false, models_present: o?.modelsPresent ?? false },
      error: null,
    },
    dictate: {
      isDictating: o?.isDictating ?? false,
      pending: o?.pending ?? false,
      error: o?.error ?? null,
    },
  };
}

function makeTts(o?: {
  modelsPresent?: boolean;
  enabled?: boolean;
  synthesizing?: boolean;
  configError?: string | null;
}) {
  return {
    config: {
      enabled: o?.enabled ?? true,
      models_present: o?.modelsPresent ?? false,
    },
    configError: o?.configError ?? null,
    synthesizing: o?.synthesizing ?? false,
  };
}

function makeRuntime(
  overrides?: Partial<{ asr: ReturnType<typeof makeAsr>; tts: ReturnType<typeof makeTts> }>,
): RuntimeState {
  return {
    asr: overrides?.asr ?? makeAsr(),
    tts: overrides?.tts ?? makeTts(),
  } as unknown as RuntimeState;
}

function renderHome() {
  return render(
    <ToastProvider>
      <HomePage />
    </ToastProvider>,
  );
}

beforeEach(() => {
  state.runtime = makeRuntime();
});

describe("HomePage 概览", () => {
  it("渲染 AI 能力卡：ASR 与 TTS 两张，纯展示无导航链接", async () => {
    renderHome();

    const capabilities = await screen.findByLabelText("AI 能力");
    expect(await within(capabilities).findByText("语音识别")).toBeInTheDocument();
    expect(within(capabilities).getByText("语音合成")).toBeInTheDocument();
    expect(within(capabilities).queryByRole("link")).not.toBeInTheDocument();
    expect(screen.getByText("查看语音识别与语音合成的状态")).toBeInTheDocument();
  });

  it("ASR：enabled 且模型在但未识别 → 已就绪（读取持久化 enabled）", async () => {
    state.runtime = makeRuntime({ asr: makeAsr({ enabled: true, modelsPresent: true }) });
    renderHome();

    const capabilities = await screen.findByLabelText("AI 能力");
    expect(await within(capabilities).findByText("已就绪")).toBeInTheDocument();
  });

  it("ASR：听写中 → 听写中（loading 蓝）", async () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ enabled: true, modelsPresent: true, isDictating: true }),
    });
    renderHome();

    const capabilities = await screen.findByLabelText("AI 能力");
    expect(await within(capabilities).findByText("听写中")).toBeInTheDocument();
  });

  it("ASR：模型在但未启用 → 未启用", async () => {
    state.runtime = makeRuntime({ asr: makeAsr({ enabled: false, modelsPresent: true }) });
    renderHome();

    const capabilities = await screen.findByLabelText("AI 能力");
    expect(await within(capabilities).findByText("未启用")).toBeInTheDocument();
  });

  it("ASR：识别出错 → 异常（红）", async () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ enabled: true, modelsPresent: true, error: "engine boom" }),
    });
    renderHome();

    const capabilities = await screen.findByLabelText("AI 能力");
    expect(await within(capabilities).findByText("异常")).toBeInTheDocument();
  });

  it("TTS：模型缺失优先于主动关闭 → 未配置（顺序沿用 ttsMeta）", async () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ enabled: true, modelsPresent: true }),
      tts: makeTts({ modelsPresent: false, enabled: false }),
    });
    renderHome();

    const capabilities = await screen.findByLabelText("AI 能力");
    expect(await within(capabilities).findByText("未配置")).toBeInTheDocument();
    expect(within(capabilities).queryByText("已关闭")).not.toBeInTheDocument();
  });

  it("TTS：已配置但主动关闭 → 已关闭", async () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ enabled: true, modelsPresent: true }),
      tts: makeTts({ modelsPresent: true, enabled: false }),
    });
    renderHome();

    const capabilities = await screen.findByLabelText("AI 能力");
    expect(await within(capabilities).findByText("已关闭")).toBeInTheDocument();
  });

  it("TTS：合成中 → 合成中", async () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ enabled: true, modelsPresent: true }),
      tts: makeTts({ modelsPresent: true, synthesizing: true }),
    });
    renderHome();

    const capabilities = await screen.findByLabelText("AI 能力");
    expect(await within(capabilities).findByText("合成中")).toBeInTheDocument();
  });

  it("TTS：配置读取失败 → 异常", async () => {
    state.runtime = makeRuntime({
      tts: makeTts({ modelsPresent: true, configError: "config boom" }),
    });
    renderHome();

    const capabilities = await screen.findByLabelText("AI 能力");
    expect(await within(capabilities).findByText("异常")).toBeInTheDocument();
  });
});
