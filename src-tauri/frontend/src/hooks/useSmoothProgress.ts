import { useEffect, useRef, useState } from "react";

/**
 * 平滑进度显示：显示值按帧向目标值逼近（而非直接跳变），消除高频下载进度事件
 * 造成的进度条抖动。下载完成（target ≥ 100）立即满格；目标不再变化时停止动画。
 * `speed` 为每帧逼近比例（0~1，越大越快）。
 */
export function useSmoothProgress(target: number | null, speed = 0.18): number {
  const [display, setDisplay] = useState(0);
  const displayRef = useRef(0);
  const speedRef = useRef(speed);
  speedRef.current = speed;

  useEffect(() => {
    if (target == null) return;
    let raf = 0;
    const tick = () => {
      const t = target;
      // 完成：直接满格并停止
      if (t >= 100) {
        displayRef.current = 100;
        setDisplay(100);
        return;
      }
      const prev = displayRef.current;
      const diff = t - prev;
      // 已逼近到误差内：定格并停止
      if (Math.abs(diff) < 0.2) {
        displayRef.current = t;
        setDisplay(t);
        return;
      }
      displayRef.current = prev + diff * speedRef.current;
      setDisplay(displayRef.current);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [target]);

  return display;
}
