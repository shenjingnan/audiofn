import { useState } from "react";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";
import { KWS_STATUS_COLOR, kwsStatus } from "./kwsMeta";

/**
 * 标题行右侧的运行控制：KWS 启用/禁用开关 + 状态反馈。
 * 开关 = 持久化的「启用 KWS」（checked 绑定 config.enabled），toggle 同时
 * 持久化偏好并开始/停止监听；右侧文字 = 「已启用 / 未启用」，
 * 圆点颜色反映真实监听状态（绿=监听中、红=错误、灰=未监听）。
 * 未下载模型且未启用时禁用；start/stop 与 toggle 在途时禁用防重复。
 */
export function KwsRunControl() {
  const { kws, device, sessionKeywords } = useRuntime();
  const [toggling, setToggling] = useState(false);
  const configured = kws.config.config?.models_present ?? false;
  const enabled = kws.config.config?.enabled ?? false;
  const status = kwsStatus(kws.config.config, kws.listening);
  const label = enabled ? "已启用" : "未启用";

  const handleToggle = (on: boolean) => {
    setToggling(true);
    (async () => {
      try {
        if (on) {
          // 先持久化 enabled，再立即开始监听（start 失败时偏好仍保留，下次启动仍会尝试）
          await kws.config.setEnabled(true);
          await kws.listening.start(device || null, sessionKeywords || null);
        } else {
          if (kws.listening.isListening) await kws.listening.stop();
          await kws.config.setEnabled(false);
        }
      } finally {
        setToggling(false);
      }
    })();
  };

  // enabled=true 时即使模型缺失也允许关掉开关（避免「已启用但模型缺失」时开关被锁死无法关闭）
  const disabled = kws.listening.pending || toggling || (!configured && !enabled);

  return (
    <div className="flex items-center gap-2.5">
      <span
        className={cn(
          "inline-flex items-center gap-1.5 text-sm font-medium",
          KWS_STATUS_COLOR[status.tone],
        )}
      >
        <span className="h-1.5 w-1.5 rounded-full bg-current" />
        {label}
      </span>
      <Switch
        aria-label="唤醒词监听开关"
        checked={enabled}
        onCheckedChange={handleToggle}
        disabled={disabled}
        trackClass="bg-emerald-500"
      />
    </div>
  );
}
