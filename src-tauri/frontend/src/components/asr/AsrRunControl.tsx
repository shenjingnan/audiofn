import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";
import { ASR_STATUS_COLOR, asrDictateStatus } from "./asrMeta";

/**
 * 标题行右侧的运行控制：状态点 + 开关，绑定免提连续听写 `asr.dictate`：
 * ON→start_asr_dictate，OFF→stop_asr_dictate。
 * 「启用」偏好是持久化的（配置区另有开关），这里只管运行时听写。
 */
export function AsrRunControl() {
  const { asr, device } = useRuntime();
  const configured = asr.config.config?.models_present ?? false;
  const { isDictating, pending } = asr.dictate;
  const status = asrDictateStatus(asr.dictate);

  const handleToggle = (on: boolean) => {
    if (on) void asr.dictate.start(device || null);
    else void asr.dictate.stop();
  };

  // 模型缺失（且未在运行）或 pending 时禁用；已在运行仍允许关掉
  const disabled = pending || (!configured && !isDictating);

  return (
    <div className="flex items-center gap-2.5">
      <span
        className={cn(
          "inline-flex items-center gap-1.5 text-sm font-medium",
          ASR_STATUS_COLOR[status.tone],
        )}
      >
        <span className="h-1.5 w-1.5 rounded-full bg-current" />
        {status.label}
      </span>
      <Switch
        aria-label="离线听写开关"
        checked={isDictating}
        onCheckedChange={handleToggle}
        disabled={disabled}
        trackClass="bg-emerald-500"
      />
    </div>
  );
}
