import { useEffect, useRef, useState } from "react";
import { onAsrDictateResult } from "@/lib/tauri";
import type { AsrResult } from "@/types/tauri";
import type { AsrSegment } from "./useAsrResults";

export interface AsrDictateResultsState {
  /** 听写已产出的整句段（最新在前；听写段无 partial，恒 final） */
  segments: AsrSegment[];
  /** 最近一次结果时间戳（前端高亮新增段用） */
  lastResultAt: number | null;
}

/**
 * 订阅独立的 `asr-dictate-result` 事件（与流式 `asr-result` 完全隔离）。
 * 听写每段整句转写完成后是最终结果，直接入段。
 */
export function useAsrDictateResults(): AsrDictateResultsState {
  const [segments, setSegments] = useState<AsrSegment[]>([]);
  const [lastResultAt, setLastResultAt] = useState<number | null>(null);
  const nextId = useRef(0);

  useEffect(() => {
    const unlisten = onAsrDictateResult((r: AsrResult) => {
      if (!r.text.trim()) return;
      const segment: AsrSegment = {
        id: nextId.current,
        text: r.text,
        at: new Date().toLocaleTimeString(),
      };
      nextId.current += 1;
      setSegments((prev) => [segment, ...prev]);
      setLastResultAt(Date.now());
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return { segments, lastResultAt };
}
