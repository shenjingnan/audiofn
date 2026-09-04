import { describe, expect, it } from "vitest";
import { toAssetUrl } from "./tauri";

function stubUserAgent(ua: string) {
  Object.defineProperty(window.navigator, "userAgent", { value: ua, configurable: true });
}

describe("toAssetUrl", () => {
  it("preserves POSIX path segments on macOS", () => {
    stubUserAgent(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)",
    );
    expect(toAssetUrl("/Users/zap/.audiofn/tts/hello.wav")).toBe(
      "asset://localhost//Users/zap/.audiofn/tts/hello.wav",
    );
  });

  it("uses the http virtual-host form on Windows (WebView2 rejects custom schemes)", () => {
    stubUserAgent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");
    expect(toAssetUrl("C:\\Users\\Administrator\\.audiofn\\tts\\hello.wav")).toBe(
      "http://asset.localhost/C%3A/Users/Administrator/.audiofn/tts/hello.wav",
    );
  });

  it("normalizes Windows separators and encodes non-ASCII segments", () => {
    stubUserAgent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");
    expect(toAssetUrl("C:\\Users\\Administrator\\我的录音 2\\你好 世界.wav")).toBe(
      "http://asset.localhost/C%3A/Users/Administrator/%E6%88%91%E7%9A%84%E5%BD%95%E9%9F%B3%202/%E4%BD%A0%E5%A5%BD%20%E4%B8%96%E7%95%8C.wav",
    );
  });
});
