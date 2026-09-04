import { describe, expect, it } from "vitest";
import { acceleratorFromEvent, formatAccelerator } from "./accelerator";

const ev = (code: string, mods: Partial<KeyboardEvent> = {}) => ({
  code,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

describe("acceleratorFromEvent", () => {
  it("Cmd+Shift+V → CmdOrCtrl+Shift+V", () => {
    expect(acceleratorFromEvent(ev("KeyV", { metaKey: true, shiftKey: true }))).toBe(
      "CmdOrCtrl+Shift+V",
    );
  });

  it("Ctrl+Alt+1（Windows/Linux 风格）→ CmdOrCtrl+Alt+1", () => {
    expect(acceleratorFromEvent(ev("Digit1", { ctrlKey: true, altKey: true }))).toBe(
      "CmdOrCtrl+Alt+1",
    );
  });

  it("支持标点/空格主键（Code 名）", () => {
    expect(acceleratorFromEvent(ev("Comma", { metaKey: true }))).toBe("CmdOrCtrl+Comma");
    expect(acceleratorFromEvent(ev("Space", { ctrlKey: true }))).toBe("CmdOrCtrl+Space");
  });

  it("裸键（无修饰键）返回 null", () => {
    expect(acceleratorFromEvent(ev("KeyZ"))).toBeNull();
  });

  it("不支持的主键返回 null", () => {
    expect(acceleratorFromEvent(ev("F5", { metaKey: true }))).toBeNull();
    expect(acceleratorFromEvent(ev("Meta", { metaKey: true }))).toBeNull();
  });
});

describe("formatAccelerator", () => {
  it("mac 显示符号：CmdOrCtrl+Shift+V → ⌘⇧V", () => {
    expect(formatAccelerator("CmdOrCtrl+Shift+V", true)).toBe("⌘⇧V");
  });

  it("非 mac 显示全名：CmdOrCtrl+Shift+V → Ctrl+Shift+V", () => {
    expect(formatAccelerator("CmdOrCtrl+Shift+V", false)).toBe("Ctrl+Shift+V");
  });

  it("标点主键显示符号：CmdOrCtrl+Comma → ⌘,", () => {
    expect(formatAccelerator("CmdOrCtrl+Comma", true)).toBe("⌘,");
  });
});
