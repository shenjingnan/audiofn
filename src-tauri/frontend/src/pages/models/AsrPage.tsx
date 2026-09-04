import { ArrowLeft } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { AsrAdvancedParams } from "@/components/asr/AsrAdvancedParams";
import { AsrBasicConfig } from "@/components/asr/AsrBasicConfig";
import { AsrDictatePanel } from "@/components/asr/AsrDictatePanel";
import { AsrModelDialog } from "@/components/asr/AsrModelDialog";
import { AsrRunControl } from "@/components/asr/AsrRunControl";
import { AsrTechnicalInfo } from "@/components/asr/AsrTechnicalInfo";
import { AsrTranscribeDialog } from "@/components/asr/AsrTranscribeDialog";
import { isStreamingAsr } from "@/components/asr/asrMeta";
import { Switch } from "@/components/ui/switch";
import { useRuntime } from "@/providers/RuntimeContext";

/**
 * 语音识别（ASR）配置页：标题行含听写开关与状态 + 启用偏好 + 基础配置 + 模型信息 + 转写文件弹窗。
 */
export function AsrPage() {
  const [switchOpen, setSwitchOpen] = useState(false);
  const [transcribeOpen, setTranscribeOpen] = useState(false);
  const { asr } = useRuntime();
  const asrEnabled = asr.config.config?.enabled ?? false;

  return (
    <div className="space-y-4">
      <Link
        to="/models"
        className="inline-flex items-center gap-1.5 text-sm text-text-secondary transition-colors hover:text-text-primary"
      >
        <ArrowLeft className="h-4 w-4" />
        模型与能力
      </Link>

      <header className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
        <h1 className="text-2xl font-semibold tracking-tight text-text-primary">
          语音识别（ASR）配置
        </h1>
        <AsrRunControl />
      </header>

      <section className="flex items-center justify-between gap-3 rounded-[16px] border border-panel-border bg-panel-background px-5 py-4">
        <div>
          <p className="text-sm font-medium text-text-primary">启用语音识别</p>
          <p className="mt-0.5 text-xs text-text-muted">关闭后听写与音频转写均不可用</p>
        </div>
        <Switch
          checked={asrEnabled}
          onCheckedChange={(v) => void asr.config.setEnabled(v)}
          aria-label="启用语音识别"
          trackClass="bg-emerald-500"
        />
      </section>

      <AsrBasicConfig
        onSwitchOpen={() => setSwitchOpen(true)}
        onTranscribeOpen={() => setTranscribeOpen(true)}
      />

      {!isStreamingAsr(asr.config.config?.model_type) && <AsrDictatePanel />}

      <AsrTechnicalInfo />

      <AsrAdvancedParams />

      <AsrModelDialog open={switchOpen} onClose={() => setSwitchOpen(false)} />

      <AsrTranscribeDialog open={transcribeOpen} onClose={() => setTranscribeOpen(false)} />
    </div>
  );
}
