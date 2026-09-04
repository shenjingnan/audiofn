import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { deriveSetupGuideIssues } from "@/components/models/setupGuide";
import type { RuntimeState } from "@/providers/RuntimeContext";
import { SetupGuideAlert } from "./SetupGuideAlert";

// 引导卡读取全局 runtime；直接 mock context 模块（同 ModelSummary.test 模式）。
const { state } = vi.hoisted(() => ({
  state: { runtime: null as RuntimeState | null },
}));

vi.mock("@/providers/RuntimeContext", () => ({
  useRuntime: () => state.runtime,
}));

// ---- runtime 切片工厂：只填 SetupGuideAlert 读取的字段 ----

function makeAsr(o?: {
  modelsPresent?: boolean;
  configError?: string | null;
  listenError?: string | null;
  configNull?: boolean;
}) {
  return {
    config: {
      config: o?.configNull ? null : { models_present: o?.modelsPresent ?? true },
      refresh: vi.fn(),
      error: o?.configError ?? null,
    },
    listening: { isListening: false, pending: false, error: o?.listenError ?? null },
  };
}

function makeTts(o?: {
  modelsPresent?: boolean;
  configError?: string | null;
  error?: string | null;
  enabled?: boolean;
  configNull?: boolean;
}) {
  return {
    config: o?.configNull
      ? null
      : { models_present: o?.modelsPresent ?? true, enabled: o?.enabled ?? true },
    configError: o?.configError ?? null,
    error: o?.error ?? null,
    synthesizing: false,
    refreshConfig: vi.fn(),
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

function renderGuide() {
  return render(
    <MemoryRouter>
      <SetupGuideAlert />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  state.runtime = makeRuntime();
});

describe("SetupGuideAlert 未配置/错误引导卡", () => {
  it("全部正常时不渲染任何引导卡", () => {
    renderGuide();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("单项未配置：直达对应配置页", () => {
    state.runtime = makeRuntime({ asr: makeAsr({ modelsPresent: false }) });
    renderGuide();
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("语音识别（ASR）尚未配置模型")).toBeInTheDocument();
    const cta = screen.getByRole("link", { name: "去配置语音识别（ASR）" });
    expect(cta).toHaveAttribute("href", "/models/asr");
  });

  it("多项未配置：每能力一个直达按钮", () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ modelsPresent: false }),
      tts: makeTts({ modelsPresent: false }),
    });
    renderGuide();
    expect(screen.getByText("2 项能力尚未配置模型")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "去配置语音识别（ASR）" })).toHaveAttribute(
      "href",
      "/models/asr",
    );
    expect(screen.getByRole("link", { name: "去配置语音合成（TTS）" })).toHaveAttribute(
      "href",
      "/models/tts",
    );
  });

  it("单项错误（listening 层）：直达对应配置页", () => {
    state.runtime = makeRuntime({ asr: makeAsr({ listenError: "engine boom" }) });
    renderGuide();
    expect(screen.getByText("语音识别（ASR）出现错误")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "查看语音识别（ASR）配置" })).toHaveAttribute(
      "href",
      "/models/asr",
    );
  });

  it("config 层错误也算错误（回归：概览页此前不展示 config 层错误）", () => {
    state.runtime = makeRuntime({ asr: makeAsr({ configError: "读取配置失败" }) });
    renderGuide();
    expect(screen.getByText("语音识别（ASR）出现错误")).toBeInTheDocument();
  });

  it("多项错误：每能力一个直达按钮", () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ listenError: "boom" }),
      tts: makeTts({ error: "boom" }),
    });
    renderGuide();
    expect(screen.getByText("2 项能力出现错误")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "查看语音识别（ASR）配置" })).toHaveAttribute(
      "href",
      "/models/asr",
    );
    expect(screen.getByRole("link", { name: "查看语音合成（TTS）配置" })).toHaveAttribute(
      "href",
      "/models/tts",
    );
  });

  it("错误与未配置并存：两张卡且错误在前", () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ modelsPresent: false }),
      tts: makeTts({ error: "load failed" }),
    });
    const { container } = renderGuide();
    const alerts = screen.getAllByRole("alert");
    expect(alerts).toHaveLength(2);
    expect(alerts[0].textContent).toContain("出现错误");
    expect(alerts[1].textContent).toContain("尚未配置模型");
    expect(container.firstChild).toBeTruthy();
  });

  it("config 未加载（null）时不误报未配置（首帧防闪烁）", () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ configNull: true }),
      tts: makeTts({ configNull: true }),
    });
    renderGuide();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("TTS 已关闭但已配置：不引导", () => {
    state.runtime = makeRuntime({ tts: makeTts({ modelsPresent: true, enabled: false }) });
    renderGuide();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("deriveSetupGuideIssues 排序：错误在前、未配置在后，组内 asr→tts", () => {
    const runtime = makeRuntime({
      asr: makeAsr({ modelsPresent: false }),
      tts: makeTts({ error: "boom" }),
    });
    const issues = deriveSetupGuideIssues(runtime);
    expect(issues.map((i) => `${i.capability}:${i.kind}`)).toEqual([
      "tts:error",
      "asr:unconfigured",
    ]);
  });
});
