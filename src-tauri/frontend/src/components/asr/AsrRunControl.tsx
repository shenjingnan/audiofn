import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";
import { ASR_STATUS_COLOR, asrDictateStatus, asrStatus, isStreamingAsr } from "./asrMeta";

/**
 * 标题行右侧的运行控制：状态点 + 开关。
 * 流式模型（zipformer）绑定实时识别 `asr.listening`：ON→start_asr_listen；
 * 离线模型（SenseVoice/Whisper）绑定免提连续听写 `asr.dictate`：ON→start_asr_dictate。
 * ASR 的「启用」偏好是持久化的（配置区另有开关），这里只管运行时识别。
 */
export function AsrRunControl() {
  const { asr, device } = useRuntime();
  const configured = asr.config.config?.models_present ?? false;
  const offline = !isStreamingAsr(asr.config.config?.model_type);

  // 按模型族选状态源：离线走听写，流式走识别（字段不同，这里归一）
  const isOn = offline ? asr.dictate.isDictating : asr.listening.isListening;
  const pending = offline ? asr.dictate.pending : asr.listening.pending;
  const status = offline ? asrDictateStatus(asr.dictate) : asrStatus(asr.listening);

  const handleToggle = (on: boolean) => {
    if (offline) {
      if (on) void asr.dictate.start(device || null);
      else void asr.dictate.stop();
    } else if (on) {
      void asr.listening.start(device || null);
    } else {
      void asr.listening.stop();
    }
  };

  // 模型缺失（且未在运行）或 pending 时禁用；已在运行仍允许关掉
  const disabled = pending || (!configured && !isOn);

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
        aria-label={offline ? "离线听写开关" : "语音识别开关"}
        checked={isOn}
        onCheckedChange={handleToggle}
        disabled={disabled}
        trackClass="bg-emerald-500"
      />
    </div>
  );
}
