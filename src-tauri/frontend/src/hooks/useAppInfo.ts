import { useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import type { AppInfo } from "@/types/tauri";

/** 读取应用版本与产品名（失败静默，非关键信息）。 */
export function useAppInfo(): AppInfo | null {
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    api
      .getAppInfo()
      .then(setInfo)
      .catch(() => {});
  }, []);

  return info;
}
