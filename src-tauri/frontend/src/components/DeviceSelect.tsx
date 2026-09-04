import { RefreshCw } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useToast } from "@/components/ui/toast";
import { api } from "@/lib/tauri";
import { useRuntime } from "@/providers/RuntimeContext";

/**
 * 全局麦克风输入设备选择（Select + 刷新）：绑定 AppRuntimeProvider 的 device/setDevice。
 * 供「设置」页与 ASR 页等任意位置复用；选择结果全局生效并持久化到 backend settings.toml。
 *
 * 听写/录音中也可直接切换设备：`set_microphone` 后端会用新设备自动重启采集，切换立即生效。
 *
 * macOS 未授权麦克风时，系统会隐藏输入设备导致列表为空（开发模式下每次重编译授权会失效）；
 * 此时显示「授权麦克风」按钮触发系统授权弹窗，授权成功后重新拉取设备列表。
 */
export function DeviceSelect() {
  const {
    devices: { devices: deviceList, refresh },
    device,
    setDevice,
    anyListening,
  } = useRuntime();

  const isMac = navigator.userAgent.includes("Mac");
  const noDevices = deviceList.length === 0;
  const [requesting, setRequesting] = useState(false);
  const toast = useToast();

  const handleRequestPermission = async () => {
    setRequesting(true);
    try {
      const granted = await api.requestMicPermission();
      if (granted) {
        toast.success("麦克风权限已授权，正在刷新设备列表");
      } else {
        toast.error("未获得麦克风权限，请在「系统设置 → 隐私与安全性 → 麦克风」中检查");
      }
      await refresh();
    } catch (e) {
      toast.error(String(e));
    } finally {
      setRequesting(false);
    }
  };

  return (
    <div className="flex shrink-0 items-center gap-2">
      <Select value={device} onValueChange={setDevice} disabled={noDevices}>
        <SelectTrigger className="w-56" aria-label="麦克风来源">
          <SelectValue placeholder={noDevices ? "未找到输入设备" : "选择麦克风"} />
        </SelectTrigger>
        <SelectContent>
          {deviceList.map((d) => (
            <SelectItem key={d} value={d}>
              {d}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Button
        variant="outline"
        size="icon"
        onClick={() => void refresh()}
        disabled={anyListening}
        aria-label="刷新设备列表"
      >
        <RefreshCw className="h-4 w-4" />
      </Button>
      {isMac && noDevices && !anyListening && (
        <Button
          variant="outline"
          size="sm"
          onClick={() => void handleRequestPermission()}
          disabled={requesting}
          aria-label="授权麦克风"
        >
          {requesting ? "请求中…" : "授权麦克风"}
        </Button>
      )}
    </div>
  );
}
