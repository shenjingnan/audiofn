import { useEffect, useRef, useState } from "react";
import { onAsrResult } from "@/lib/tauri";
import type { AsrResult } from "@/types/tauri";

export interface AsrSegment {
  id: number;
  text: string;
  at: string;
}

export interface AsrResultsState {
  /** 当前正在识别的部分文本（实时更新） */
  partial: string;
  /** 已断句的最终转写段（最新在前） */
  segments: AsrSegment[];
}

/**
 * 订阅 `asr-result` 事件：`is_final` 的最终结果入段，否则作为实时部分文本。
 */
export function useAsrResults(): AsrResultsState {
  const [partial, setPartial] = useState("");
  const [segments, setSegments] = useState<AsrSegment[]>([]);
  const nextId = useRef(0);

  useEffect(() => {
    const unlisten = onAsrResult((r: AsrResult) => {
      if (r.is_final) {
        if (r.text.trim()) {
          const segment: AsrSegment = {
            id: nextId.current,
            text: r.text,
            at: new Date().toLocaleTimeString(),
          };
          nextId.current += 1;
          setSegments((prev) => [segment, ...prev]);
        }
        setPartial("");
      } else {
        setPartial(r.text);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return { partial, segments };
}
