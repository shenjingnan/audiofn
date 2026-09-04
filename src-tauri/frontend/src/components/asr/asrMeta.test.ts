import { describe, expect, it } from "vitest";
import { asrDictateStatus, asrModelKindLabel, isStreamingAsr } from "./asrMeta";

describe("asrModelKindLabel", () => {
  it("族徽标文案", () => {
    expect(asrModelKindLabel("zipformer")).toBe("流式 Zipformer");
    expect(asrModelKindLabel("paraformer")).toBe("流式 Paraformer");
    expect(asrModelKindLabel("sensevoice")).toBe("SenseVoice");
    expect(asrModelKindLabel("whisper")).toBe("Whisper");
    expect(asrModelKindLabel("qwen3_asr")).toBe("Qwen3-ASR");
    expect(asrModelKindLabel("unknown")).toBe("ASR");
  });
});

describe("isStreamingAsr", () => {
  it("zipformer / paraformer 或缺省（老配置）→ 流式", () => {
    expect(isStreamingAsr("zipformer")).toBe(true);
    expect(isStreamingAsr("paraformer")).toBe(true);
    expect(isStreamingAsr(null)).toBe(true);
    expect(isStreamingAsr(undefined)).toBe(true);
  });

  it("sensevoice / whisper / qwen3_asr → 离线（仅转写文件）", () => {
    expect(isStreamingAsr("sensevoice")).toBe(false);
    expect(isStreamingAsr("whisper")).toBe(false);
    expect(isStreamingAsr("qwen3_asr")).toBe(false);
  });
});

describe("asrDictateStatus", () => {
  it("状态机：错误 > 启动中 > 听写中 > 未听写", () => {
    expect(asrDictateStatus({ isDictating: false, pending: false, error: "x" }).label).toBe(
      "错误",
    );
    expect(asrDictateStatus({ isDictating: false, pending: true, error: null }).label).toBe(
      "启动中",
    );
    expect(asrDictateStatus({ isDictating: true, pending: false, error: null }).label).toBe(
      "听写中",
    );
    expect(asrDictateStatus({ isDictating: false, pending: false, error: null }).label).toBe(
      "未听写",
    );
  });
});
