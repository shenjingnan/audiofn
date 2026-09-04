import { useState } from "react";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";
import { TTS_STATUS_COLOR, ttsStatus } from "./ttsMeta";

/**
 * 标题行右侧的运行控制：状态点 + 「启用语音合成」开关。
 * 开关真实绑定持久化的 `[tts].enabled`（set_tts_enabled），只负责是否允许后续 TTS 调用；
 * 当前合成任务由 TestDialog 的「停止」管理，故合成期间禁用此开关（不承担停止语义）。
 */
export function TtsRunControl() {
  const { tts } = useRuntime();
  const [toggling, setToggling] = useState(false);
  const enabled = tts.config?.enabled ?? true;
  const status = ttsStatus(tts.config, tts.synthesizing, tts.configError);

  const handleToggle = (on: boolean) => {
    setToggling(true);
    (async () => {
      try {
        await tts.setEnabled(on);
      } finally {
        setToggling(false);
      }
    })();
  };

  return (
    <div className="flex items-center gap-2.5">
      <span
        className={cn(
          "inline-flex items-center gap-1.5 text-sm font-medium",
          TTS_STATUS_COLOR[status.tone],
        )}
      >
        <span className="h-1.5 w-1.5 rounded-full bg-current" />
        {status.label}
      </span>
      <Switch
        aria-label="语音合成开关"
        checked={enabled}
        onCheckedChange={handleToggle}
        disabled={toggling || tts.synthesizing}
        trackClass="bg-emerald-500"
      />
    </div>
  );
}
