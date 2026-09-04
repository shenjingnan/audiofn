import { describe, expect, it } from "vitest";
import { toAssetUrl } from "./tauri";

function stubUserAgent(ua: string) {
  Object.defineProperty(window.navigator, "userAgent", { value: ua, configurable: true });
}

describe("toAssetUrl", () => {
  it("preserves POSIX path segments for relative Live2D resources on macOS", () => {
    stubUserAgent(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)",
    );
    expect(toAssetUrl("/Users/zap/model/cat.model3.json")).toBe(
      "asset://localhost//Users/zap/model/cat.model3.json",
    );
  });

  it("uses the http virtual-host form on Windows (WebView2 rejects custom schemes)", () => {
    stubUserAgent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");
    expect(
      toAssetUrl("C:\\Users\\Administrator\\.zapmomo\\companions\\companion-1\\cat.model3.json"),
    ).toBe(
      "http://asset.localhost/C%3A/Users/Administrator/.zapmomo/companions/companion-1/cat.model3.json",
    );
  });

  it("normalizes Windows separators and encodes non-ASCII segments", () => {
    stubUserAgent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");
    expect(toAssetUrl("C:\\Users\\Administrator\\白发天使 2\\曲奇小羊.model3.json")).toBe(
      "http://asset.localhost/C%3A/Users/Administrator/%E7%99%BD%E5%8F%91%E5%A4%A9%E4%BD%BF%202/%E6%9B%B2%E5%A5%87%E5%B0%8F%E7%BE%8A.model3.json",
    );
  });
});
