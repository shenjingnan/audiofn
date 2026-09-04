import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAsrDictateResults } from "./useAsrDictateResults";

const { resultHandlers } = vi.hoisted(() => {
  const handlers: ((r: { text: string; is_final: boolean }) => void)[] = [];
  return { resultHandlers: handlers };
});

vi.mock("@/lib/tauri", () => ({
  onAsrDictateResult: vi.fn((cb: (r: { text: string; is_final: boolean }) => void) => {
    resultHandlers.push(cb);
    return Promise.resolve(() => {});
  }),
}));

function emitResult(text: string) {
  act(() => resultHandlers.forEach((h) => h({ text, is_final: true })));
}

beforeEach(() => {
  resultHandlers.length = 0;
});

describe("useAsrDictateResults", () => {
  it("听写结果入段（最新在前），空文本跳过", () => {
    const { result } = renderHook(() => useAsrDictateResults());
    emitResult("第一句");
    emitResult("第二句");
    emitResult("   "); // 空文本跳过

    expect(result.current.segments.map((s) => s.text)).toEqual(["第二句", "第一句"]);
    expect(result.current.segments[0].at).toBeTruthy();
    expect(result.current.lastResultAt).not.toBeNull();
  });

  it("初始为空", () => {
    const { result } = renderHook(() => useAsrDictateResults());
    expect(result.current.segments).toEqual([]);
    expect(result.current.lastResultAt).toBeNull();
  });
});
