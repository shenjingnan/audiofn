import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const { invokeMock, listeners } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listeners: new Map<string, (e: { payload: unknown }) => void>(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (e: { payload: unknown }) => void) => {
    listeners.set(event, handler);
    return Promise.resolve(() => {});
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  })),
}));

const ASR_CONFIG = {
  model_dir: "/home/user/.zapmomo/models/sherpa-onnx-streaming-zipformer",
  provider: "cpu",
  num_threads: 4,
  sample_rate: 16000,
  enabled: false,
  models_present: false,
  punctuation_present: false,
  model_downloading: false,
  settings_path: "/home/user/.zapmomo/settings.toml",
};

const TTS_CONFIG = {
  model_dir: "/home/user/.zapmomo/models/qwen3-tts",
  provider: "cpu",
  num_threads: 4,
  enabled: true,
  models_present: false,
  model_downloading: false,
  settings_path: "/home/user/.zapmomo/settings.toml",
};

/** 可变 ASR/TTS 配置（引导卡「全部正常不渲染」等用例需置 models_present）。 */
let asrConfig: typeof ASR_CONFIG;
let ttsConfig: typeof TTS_CONFIG;
/** 模拟后端持久化的麦克风（get_microphone / set_microphone）。 */
let mic = "";
/** 模拟后端可枚举的输入设备（置空以测试 macOS 未授权场景）。 */
let devices: string[];

/** 渲染 App 并定位到指定路由（默认模型概览页）。 */
function renderApp(initialPath = "/models") {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <App />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  invokeMock.mockReset();
  listeners.clear();
  asrConfig = { ...ASR_CONFIG };
  ttsConfig = { ...TTS_CONFIG };
  mic = "";
  devices = ["内置麦克风", "USB 麦克风"];

  invokeMock.mockImplementation(
    (cmd: string, args?: { enabled?: boolean; mic?: string; device?: string | null }) => {
      switch (cmd) {
        case "get_app_info":
          return Promise.resolve({ version: "0.1.4", product_name: "ZapMomo" });
        case "list_devices":
          return Promise.resolve(devices);
        case "request_mic_permission":
          return Promise.resolve(true);
        case "get_microphone":
          return Promise.resolve(mic);
        case "set_microphone":
          mic = args?.mic ?? "";
          return Promise.resolve(undefined);
        case "get_asr_config":
          return Promise.resolve({ ...asrConfig });
        case "set_asr_enabled":
          asrConfig = { ...asrConfig, enabled: args?.enabled ?? false };
          return Promise.resolve(undefined);
        case "start_asr_listen":
        case "stop_asr_listen":
        case "is_asr_listening":
          return cmd === "is_asr_listening" ? Promise.resolve(false) : Promise.resolve(undefined);
        case "get_tts_config":
          return Promise.resolve({ ...ttsConfig });
        case "set_tts_enabled":
          ttsConfig = { ...ttsConfig, enabled: args?.enabled ?? false };
          return Promise.resolve(undefined);
        case "list_tts_voices":
          return Promise.resolve([]);
        case "list_model_library":
          return Promise.resolve([]);
        case "get_shortcuts":
          return Promise.resolve({});
        case "get_hide_dock_icon":
          return Promise.resolve(false);
        case "get_autostart":
          return Promise.resolve(false);
        case "get_storage_info":
        case "get_storage_prompt":
          return Promise.resolve(null);
        case "get_system_resources":
          return Promise.resolve(null);
        default:
          // 已裁剪能力（KWS/LLM/声纹/语音会话/伴侣）的命令：后端不再注册，返回空。
          return Promise.resolve(undefined);
      }
    },
  );
});

describe("App（路由收敛：概览 / 模型 / 设置）", () => {
  it("渲染 Sidebar 导航（概览 / 模型 / 设置）与模型概览页", async () => {
    renderApp("/models");
    expect(screen.getByAltText("ZapMomo")).toBeInTheDocument();
    expect(screen.getByText("概览")).toBeInTheDocument();
    expect(screen.getByText("设置")).toBeInTheDocument();
    expect(screen.getByText("模型摘要")).toBeInTheDocument();
    // 已裁剪页面不再出现在导航
    expect(screen.queryByText("伙伴")).not.toBeInTheDocument();
    expect(screen.queryByText("插件集成")).not.toBeInTheDocument();
    expect(screen.queryByText("对话记录")).not.toBeInTheDocument();
  });

  it("默认路由重定向到 /home（概览页）", async () => {
    renderApp("/");
    expect(await screen.findByText("AI 能力")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "概览" })).toBeInTheDocument();
  });

  it("概览页：AI 能力卡只有 ASR 与 TTS 两张（纯展示）", async () => {
    renderApp("/home");

    const capabilities = await screen.findByLabelText("AI 能力");
    expect(await within(capabilities).findByText("语音识别")).toBeInTheDocument();
    expect(within(capabilities).getByText("语音合成")).toBeInTheDocument();
    expect(within(capabilities).queryByText("唤醒词")).not.toBeInTheDocument();
    expect(within(capabilities).queryByRole("link")).not.toBeInTheDocument();
  });

  it("概览页 ASR 开关调用 start_asr_listen", async () => {
    asrConfig = { ...ASR_CONFIG, models_present: true };
    const user = userEvent.setup();
    renderApp("/models");

    await user.click(await screen.findByRole("switch", { name: "语音识别（ASR）开关" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("start_asr_listen", { device: null });
    });
  });

  it("概览页语音合成开关调用 set_tts_enabled", async () => {
    const user = userEvent.setup();
    renderApp("/models");

    await user.click(await screen.findByRole("switch", { name: "语音合成（TTS）开关" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_tts_enabled", { enabled: false });
    });
  });

  it("概览页引导卡：默认全未配置 → 每项能力一个直达按钮", async () => {
    renderApp("/models");
    expect(await screen.findByText("2 项能力尚未配置模型")).toBeInTheDocument();
    const cases: Array<[string, string]> = [
      ["去配置语音识别（ASR）", "/models/asr"],
      ["去配置语音合成（TTS）", "/models/tts"],
    ];
    for (const [name, href] of cases) {
      expect(screen.getByRole("link", { name })).toHaveAttribute("href", href);
    }
  });

  it("概览页引导卡：全部配置正常时不渲染", async () => {
    asrConfig = { ...asrConfig, models_present: true };
    ttsConfig = { ...ttsConfig, models_present: true };
    renderApp("/models");
    // 等待配置加载完成（「未配置模型」span 消失）后再断言无引导卡。
    await waitFor(() => {
      expect(screen.queryByText("尚未配置模型")).not.toBeInTheDocument();
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("ASR 配置页可达：渲染标题与返回入口", async () => {
    renderApp("/models/asr");
    expect(await screen.findByRole("heading", { name: "语音识别（ASR）配置" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "模型与能力" })).toHaveAttribute("href", "/models");
  });

  it("TTS 配置页可达：渲染标题与返回入口", async () => {
    renderApp("/models/tts");
    expect(await screen.findByRole("heading", { name: "语音合成（TTS）配置" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "模型与能力" })).toHaveAttribute("href", "/models");
  });

  it("已裁剪路由（kws/llm/speaker/integrations/chat/companion）不再渲染任何页面", () => {
    for (const path of [
      "/models/kws",
      "/models/llm",
      "/models/speaker",
      "/integrations",
      "/chat",
      "/companion",
    ]) {
      const { unmount } = renderApp(path);
      // 无匹配路由：主面板只有壳层（无页面标题），也不崩
      expect(screen.queryByRole("heading")).not.toBeInTheDocument();
      unmount();
    }
  });

  it("设置页可切换是否隐藏 Dock / Cmd+Tab 图标", async () => {
    const user = userEvent.setup();
    renderApp("/settings");

    const toggle = await screen.findByRole("switch", { name: "隐藏应用图标" });
    expect(toggle).toHaveAttribute("aria-checked", "false");

    await user.click(toggle);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_hide_dock_icon", { hide: true });
    });
  });

  it("设置页可选择麦克风并持久化到后端", async () => {
    const user = userEvent.setup();
    renderApp("/settings");

    await user.click(await screen.findByRole("combobox", { name: "麦克风来源" }));
    await user.click(await screen.findByRole("option", { name: "USB 麦克风" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_microphone", { mic: "USB 麦克风" });
    });
  });

  it("设置页刷新设备按钮重新调用 list_devices", async () => {
    const user = userEvent.setup();
    renderApp("/settings");

    await user.click(await screen.findByRole("button", { name: "刷新设备列表" }));

    await waitFor(() => {
      const calls = invokeMock.mock.calls.filter((c) => c[0] === "list_devices");
      expect(calls.length).toBeGreaterThanOrEqual(2);
    });
  });

  it("设置页无设备时（macOS 未授权）显示授权按钮并触发权限请求", async () => {
    const user = userEvent.setup();
    devices = [];
    // 模拟 macOS WebView 的 userAgent（授权按钮仅 macOS 显示）
    const uaDesc = Object.getOwnPropertyDescriptor(navigator, "userAgent");
    Object.defineProperty(navigator, "userAgent", {
      value: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
      configurable: true,
    });
    try {
      renderApp("/settings");

      const grantBtn = await screen.findByRole("button", { name: "授权麦克风" });
      expect(screen.getByRole("combobox", { name: "麦克风来源" })).toBeDisabled();

      await user.click(grantBtn);

      await waitFor(() => {
        expect(invokeMock).toHaveBeenCalledWith("request_mic_permission");
      });
      // 授权后重新拉取设备列表
      await waitFor(() => {
        const calls = invokeMock.mock.calls.filter((c) => c[0] === "list_devices");
        expect(calls.length).toBeGreaterThanOrEqual(2);
      });
    } finally {
      if (uaDesc) Object.defineProperty(navigator, "userAgent", uaDesc);
    }
  });

  it("设置页点击「重启」按钮调用 restart_app", async () => {
    const user = userEvent.setup();
    renderApp("/settings");

    await user.click(await screen.findByRole("button", { name: "重启" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("restart_app");
    });
  });
});
