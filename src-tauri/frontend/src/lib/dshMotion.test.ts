import { describe, expect, it } from "vitest";
import { pickMotionGroup } from "./dshMotion";

describe("pickMotionGroup", () => {
  it("按事件类型提示词匹配组名（大小写不敏感子串）", () => {
    expect(pickMotionGroup(["Idle", "TapBody", "FlickHead"], "task-started")).toBe("TapBody");
    expect(pickMotionGroup(["Idle", "Happy", "Tap"], "task-finished")).toBe("Happy");
    expect(pickMotionGroup(["idle", "Sad"], "task-failed")).toBe("Sad");
  });

  it("未命中返回 null（调用方静默跳过）", () => {
    expect(pickMotionGroup(["Idle"], "task-finished")).toBeNull();
    expect(pickMotionGroup([], "task-started")).toBeNull();
  });
});
