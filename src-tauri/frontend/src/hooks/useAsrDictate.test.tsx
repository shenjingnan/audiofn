import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAsrDictate } from "./useAsrDictate";

const { startMock, stopMock, isDictatingMock, startedHandlers, stoppedHandlers } = vi.hoisted(
  () => {
    const started: ((p: { error?: string | null }) => void)[] = [];
    const stopped: ((p: { error?: string | null }) => void)[] = [];
    return {
      startMock: vi.fn().mockResolvedValue(undefined),
      stopMock: vi.fn().mockResolvedValue(undefined),
      isDictatingMock: vi.fn().mockResolvedValue(false),
      startedHandlers: started,
      stoppedHandlers: stopped,
    };
  },
);

vi.mock("@/lib/tauri", () => ({
  api: {
    isAsrDictating: isDictatingMock,
    startAsrDictate: startMock,
    stopAsrDictate: stopMock,
  },
  onAsrDictateStarted: vi.fn((cb: (p: { error?: string | null }) => void) => {
    startedHandlers.push(cb);
    return Promise.resolve(() => {});
  }),
  onAsrDictateStopped: vi.fn((cb: (p: { error?: string | null }) => void) => {
    stoppedHandlers.push(cb);
    return Promise.resolve(() => {});
  }),
}));

function emitStarted(payload: { error?: string | null } = {}) {
  act(() => startedHandlers.forEach((h) => h(payload)));
}

function emitStopped(payload: { error?: string | null } = {}) {
  act(() => stoppedHandlers.forEach((h) => h(payload)));
}

function Probe() {
  const { isDictating, pending, error, start, stop } = useAsrDictate();
  return (
    <div>
      <span data-testid="dictating">{String(isDictating)}</span>
      <span data-testid="pending">{String(pending)}</span>
      <span data-testid="error">{error ?? ""}</span>
      <button type="button" data-testid="start" onClick={() => void start("mic")}>
        start
      </button>
      <button type="button" data-testid="stop" onClick={() => void stop()}>
        stop
      </button>
    </div>
  );
}

beforeEach(() => {
  startMock.mockClear();
  stopMock.mockClear();
  isDictatingMock.mockClear();
  isDictatingMock.mockResolvedValue(false);
  startedHandlers.length = 0;
  stoppedHandlers.length = 0;
});

describe("useAsrDictate", () => {
  it("挂载时回读后端状态；start/stop 调用对应 command 并翻转状态", async () => {
    const user = userEvent.setup();
    render(<Probe />);
    await screen.findByTestId("dictating");
    expect(isDictatingMock).toHaveBeenCalled();

    await user.click(screen.getByTestId("start"));
    await waitFor(() => {
      expect(startMock).toHaveBeenCalledWith({ device: "mic" });
    });
    expect(screen.getByTestId("dictating").textContent).toBe("true");

    await user.click(screen.getByTestId("stop"));
    await waitFor(() => {
      expect(stopMock).toHaveBeenCalled();
    });
    expect(screen.getByTestId("dictating").textContent).toBe("false");
  });

  it("start 失败显示错误且不置听写中", async () => {
    startMock.mockRejectedValueOnce(new Error("离线模型未就绪"));
    const user = userEvent.setup();
    render(<Probe />);
    await user.click(screen.getByTestId("start"));
    expect(await screen.findByTestId("error")).toHaveTextContent("离线模型未就绪");
    expect(screen.getByTestId("dictating").textContent).toBe("false");
  });

  it("asr-dictate-stopped 事件复位并透传错误；started 事件置位", async () => {
    render(<Probe />);
    await screen.findByTestId("dictating");

    emitStarted();
    expect(screen.getByTestId("dictating").textContent).toBe("true");

    emitStopped({ error: "VAD 模型下载失败" });
    expect(screen.getByTestId("dictating").textContent).toBe("false");
    expect(screen.getByTestId("error").textContent).toBe("VAD 模型下载失败");
  });
});
