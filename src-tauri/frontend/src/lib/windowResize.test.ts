import { describe, expect, it } from "vitest";
import { centeredResizeTarget } from "./windowResize";

describe("centeredResizeTarget", () => {
  it("keeps the top-left unchanged when the target size equals the current size", () => {
    expect(centeredResizeTarget({ x: 100, y: 200, width: 300, height: 400 }, 300, 400)).toEqual({
      x: 100,
      y: 200,
    });
  });

  it("shifts the top-left up-left when enlarging so the center stays fixed", () => {
    // Center (250, 400); enlarging to 500×600 → top-left (0, 100).
    expect(centeredResizeTarget({ x: 100, y: 200, width: 300, height: 400 }, 500, 600)).toEqual({
      x: 0,
      y: 100,
    });
  });

  it("shifts the top-left down-right when shrinking so the center stays fixed", () => {
    // Center (250, 400); shrinking to 200×300 → top-left (150, 250).
    expect(centeredResizeTarget({ x: 100, y: 200, width: 300, height: 400 }, 200, 300)).toEqual({
      x: 150,
      y: 250,
    });
  });

  it("rounds half-pixel deltas to the nearest integer", () => {
    expect(centeredResizeTarget({ x: 0, y: 0, width: 301, height: 401 }, 300, 400)).toEqual({
      x: 1,
      y: 1,
    });
  });
});
