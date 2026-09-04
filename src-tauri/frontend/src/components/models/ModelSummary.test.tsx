import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RuntimeState } from "@/providers/RuntimeContext";
import { ModelSummary } from "./ModelSummary";

// 模型摘要读取全局 runtime；直接 mock context 模块，避免挂载完整 App / mock tauri invoke。
const { state } = vi.hoisted(() => ({
  // 可变 runtime 快照：单个用例按需替换 asr/tts 切片（贴近真实 RuntimeState）。
  state: { runtime: null as RuntimeState | null },
}));

vi.mock("@/providers/RuntimeContext", () => ({
  useRuntime: () => state.runtime,
}));

// 选择模型弹窗 stub：避免 useAsrModelSwitch / useTtsModelSwitch 的 useToast/invoke 依赖。
vi.mock("@/components/asr/AsrModelDialog", () => ({
  AsrModelDialog: () => null,
}));

vi.mock("@/components/tts/TtsModelDialog", () => ({
  TtsModelDialog: () => null,
}));

// ---- runtime 切片工厂：只填 ModelSummary 读取的字段（其余方法 vi.fn() 兜底）----

function makeAsr(o?: {
  enabled?: boolean;
  modelsPresent?: boolean;
  isDictating?: boolean;
  pending?: boolean;
  error?: string | null;
}) {
  return {
    config: {
      config: {
        enabled: o?.enabled ?? false,
        models_present: o?.modelsPresent ?? false,
        model_dir: "/zap/.zapmomo/models/asr",
      },
      refresh: vi.fn(),
      setEnabled: vi.fn(),
      error: null,
    },
    dictate: {
      isDictating: o?.isDictating ?? false,
      pending: o?.pending ?? false,
      error: o?.error ?? null,
    },
  };
}

function makeTts(o?: { modelsPresent?: boolean; enabled?: boolean; synthesizing?: boolean }) {
  return {
    config: {
      enabled: o?.enabled ?? true,
      models_present: o?.modelsPresent ?? false,
      model_dir: "/zap/.zapmomo/models/tts",
    },
    configError: null,
    synthesizing: o?.synthesizing ?? false,
    refreshConfig: vi.fn(),
    setEnabled: vi.fn(),
  };
}

function makeRuntime(
  overrides?: Partial<{ asr: ReturnType<typeof makeAsr>; tts: ReturnType<typeof makeTts> }>,
): RuntimeState {
  return {
    asr: overrides?.asr ?? makeAsr(),
    tts: overrides?.tts ?? makeTts(),
    device: null,
  } as unknown as RuntimeState;
}

/** 各摘要行的 aria-label（SummaryRow 用 aria-label=`配置${row.name}`）。 */
const ROW_NAME = {
  asr: "配置语音识别（ASR）",
  tts: "配置语音合成（TTS）",
} as const;

function rowFor(key: keyof typeof ROW_NAME): HTMLElement {
  return screen.getByRole("link", { name: ROW_NAME[key] });
}

/** 断言某行显示指定状态文本（可选断言状态色 class，映射 STATUS_COLOR）。 */
function expectRowStatus(row: HTMLElement, text: string, toneClass?: string) {
  // 未配置时模型名 `<p>` 与状态 `<span>` 都会显示「未配置模型」，用 selector 精确定位状态 span。
  const status = within(row).getByText(text, { selector: "span" });
  if (toneClass) expect(status).toHaveClass(toneClass);
}

function renderSummary() {
  return render(
    <MemoryRouter>
      <ModelSummary />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  state.runtime = makeRuntime();
});

describe("ModelSummary 模型摘要状态", () => {
  it("ASR：听写出错 → 错误", () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ modelsPresent: true, enabled: true, error: "engine boom" }),
    });
    renderSummary();
    expectRowStatus(rowFor("asr"), "错误", "text-red-600");
  });

  it("ASR：启动中 → 启动中", () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ modelsPresent: true, enabled: true, pending: true }),
    });
    renderSummary();
    expectRowStatus(rowFor("asr"), "启动中", "text-blue-600");
  });

  it("ASR：听写中 → 听写中", () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ modelsPresent: true, enabled: true, isDictating: true }),
    });
    renderSummary();
    expectRowStatus(rowFor("asr"), "听写中");
  });

  it("ASR：enabled 且模型在但未识别 → 已就绪（回归：此前误显示未启用）", () => {
    state.runtime = makeRuntime({ asr: makeAsr({ modelsPresent: true, enabled: true }) });
    renderSummary();
    expectRowStatus(rowFor("asr"), "已就绪", "text-emerald-600");
  });

  it("ASR：模型在但未启用 → 未启用（回归：此前误显示已就绪）", () => {
    state.runtime = makeRuntime({ asr: makeAsr({ modelsPresent: true, enabled: false }) });
    renderSummary();
    expectRowStatus(rowFor("asr"), "未启用", "text-text-muted");
  });

  it("ASR：无模型 → 未配置模型", () => {
    renderSummary();
    expectRowStatus(rowFor("asr"), "未配置模型");
  });

  it("TTS：主动关闭 → 已关闭", () => {
    state.runtime = makeRuntime({ tts: makeTts({ modelsPresent: true, enabled: false }) });
    renderSummary();
    expectRowStatus(rowFor("tts"), "已关闭");
  });

  it("TTS：合成中 → 合成中（loading）", () => {
    state.runtime = makeRuntime({
      asr: makeAsr({ modelsPresent: true, enabled: true }),
      tts: makeTts({ modelsPresent: true, synthesizing: true }),
    });
    renderSummary();
    expectRowStatus(rowFor("tts"), "合成中", "text-blue-600");
  });

  it("默认全未配置：两行状态均显示未配置模型", () => {
    renderSummary();
    // 每行模型名 `<p>` 也是「未配置模型」，这里只统计状态 span（2 行各 1 个）。
    expect(screen.getAllByText("未配置模型", { selector: "span" })).toHaveLength(2);
  });
});

describe("ModelSummary 摘要行开关", () => {
  it("ASR：开启 → 持久化 enabled（听写由配置页运行开关控制）", async () => {
    const user = userEvent.setup();
    const asr = makeAsr({ modelsPresent: true, enabled: false });
    state.runtime = makeRuntime({ asr });
    renderSummary();

    await user.click(screen.getByRole("switch", { name: "语音识别（ASR）开关" }));

    expect(asr.config.setEnabled).toHaveBeenCalledWith(true);
  });

  it("ASR：关闭 → 持久化 disabled", async () => {
    const user = userEvent.setup();
    const asr = makeAsr({ modelsPresent: true, enabled: true });
    state.runtime = makeRuntime({ asr });
    renderSummary();

    await user.click(screen.getByRole("switch", { name: "语音识别（ASR）开关" }));

    expect(asr.config.setEnabled).toHaveBeenCalledWith(false);
  });

  it("TTS：点击开关调用 setEnabled(false)", async () => {
    const user = userEvent.setup();
    const tts = makeTts({ modelsPresent: true, enabled: true });
    state.runtime = makeRuntime({ tts });
    renderSummary();

    await user.click(screen.getByRole("switch", { name: "语音合成（TTS）开关" }));

    expect(tts.setEnabled).toHaveBeenCalledWith(false);
  });
});
