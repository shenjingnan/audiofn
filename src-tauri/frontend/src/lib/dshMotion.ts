import type { DshEventInfo } from "@/types/tauri";

/** 事件类型 → motion 组名提示词（大小写不敏感子串匹配） */
const MOTION_HINTS: Record<DshEventInfo["type"], string[]> = {
  "task-started": ["tap", "flick", "greet", "hello"],
  "task-finished": ["happy", "smile", "joy", "win", "dance"],
  "task-failed": ["sad", "cry", "angry", "shock"],
  "task-interrupted": ["idle", "surprise", "think"],
};

/**
 * 从模型可用 motion 组里挑一个匹配的（第一个命中提示词的组）。
 * 模型组名千差万别，匹配不到返回 null（调用方静默跳过）。
 */
export function pickMotionGroup(groups: string[], type: DshEventInfo["type"]): string | null {
  const hints = MOTION_HINTS[type] ?? [];
  for (const hint of hints) {
    const idx = groups.findIndex((g) => g.toLowerCase().includes(hint));
    if (idx >= 0) return groups[idx];
  }
  return null;
}
