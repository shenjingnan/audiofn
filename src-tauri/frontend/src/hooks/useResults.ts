import { useEffect, useRef, useState } from "react";
import { onKeywordDetected } from "@/lib/tauri";
import type { KwsResult } from "@/types/tauri";

export interface DetectionEntry {
  id: number;
  keyword: string;
  startTime: number;
  at: string;
}

/** 订阅 `kws-detected` 事件，把检测结果倒序（最新在前）累积。 */
export function useResults(): DetectionEntry[] {
  const [results, setResults] = useState<DetectionEntry[]>([]);
  const nextId = useRef(0);

  useEffect(() => {
    const unlisten = onKeywordDetected((r: KwsResult) => {
      const entry: DetectionEntry = {
        id: nextId.current,
        keyword: r.keyword,
        startTime: r.start_time,
        at: new Date().toLocaleTimeString(),
      };
      nextId.current += 1;
      setResults((prev) => [entry, ...prev]);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return results;
}
