import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ToastProvider } from "@/components/ui/toast";
import { ShortcutsSection } from "./ShortcutsSection";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

function renderSection() {
  return render(
    <ToastProvider>
      <ShortcutsSection />
    </ToastProvider>,
  );
}

const keyDown = (code: string, mods: Partial<KeyboardEvent> = {}) =>
  fireEvent.keyDown(window, { code, key: code, ...mods });

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === "get_shortcuts") return Promise.resolve({});
    return Promise.resolve();
  });
});

describe("ShortcutsSection", () => {
  it("挂载时读取已绑定快捷键并展示", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_shortcuts") return Promise.resolve({ open_settings: "CmdOrCtrl+Shift+O" });
      return Promise.resolve();
    });
    renderSection();
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_shortcuts"));
    // 已绑定显示 accelerator（含主键 O）
    expect(screen.getByLabelText("设置 打开设置 快捷键").textContent).toContain("O");
  });

  it("录制：点击后按键组合 → 调 set_shortcut 并更新展示", async () => {
    renderSection();
    const btn = await screen.findByLabelText("设置 打开设置 快捷键");
    fireEvent.click(btn);
    expect(btn.textContent).toContain("按下组合键");
    keyDown("KeyO", { metaKey: true, shiftKey: true });
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("set_shortcut", {
        action: "open_settings",
        accelerator: "CmdOrCtrl+Shift+O",
      }),
    );
    await waitFor(() =>
      expect(screen.getByLabelText("设置 打开设置 快捷键").textContent).toContain("O"),
    );
  });

  it("Esc 取消录制且不发请求", async () => {
    renderSection();
    const btn = await screen.findByLabelText("设置 打开设置 快捷键");
    fireEvent.click(btn);
    keyDown("Escape");
    keyDown("KeyO", { metaKey: true });
    expect(invokeMock).not.toHaveBeenCalledWith("set_shortcut", expect.anything());
    expect(screen.getByLabelText("设置 打开设置 快捷键").textContent).toContain("未设置");
  });

  it("裸按键被忽略（等待有效组合）", async () => {
    renderSection();
    const btn = await screen.findByLabelText("设置 打开设置 快捷键");
    fireEvent.click(btn);
    keyDown("KeyO");
    expect(invokeMock).not.toHaveBeenCalledWith("set_shortcut", expect.anything());
    expect(screen.getByLabelText("设置 打开设置 快捷键").textContent).toContain("按下组合键");
  });

  it("后端注册失败：显示错误且原绑定不变", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_shortcuts") return Promise.resolve({ open_settings: "CmdOrCtrl+Shift+O" });
      if (cmd === "set_shortcut") return Promise.reject("注册失败，可能已被其他应用占用");
      return Promise.resolve();
    });
    renderSection();
    await screen.findByLabelText("设置 打开设置 快捷键");
    fireEvent.click(screen.getByLabelText("设置 打开设置 快捷键"));
    keyDown("KeyP", { metaKey: true, shiftKey: true });
    await waitFor(() => expect(screen.getByText(/注册失败/)).toBeTruthy());
    expect(screen.getByLabelText("设置 打开设置 快捷键").textContent).toContain("O");
  });

  it("清除：调 clear_shortcut 并回到未设置", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_shortcuts") return Promise.resolve({ open_settings: "CmdOrCtrl+Shift+O" });
      return Promise.resolve();
    });
    renderSection();
    fireEvent.click(await screen.findByLabelText("清除 打开设置 快捷键"));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("clear_shortcut", {
        action: "open_settings",
      }),
    );
    await waitFor(() =>
      expect(screen.getByLabelText("设置 打开设置 快捷键").textContent).toContain("未设置"),
    );
  });
});
