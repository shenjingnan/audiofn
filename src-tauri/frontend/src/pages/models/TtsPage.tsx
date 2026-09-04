import { ArrowLeft } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { TtsAdvancedParams } from "@/components/tts/TtsAdvancedParams";
import { TtsBasicConfig } from "@/components/tts/TtsBasicConfig";
import { TtsModelDialog } from "@/components/tts/TtsModelDialog";
import { TtsModelInfo } from "@/components/tts/TtsModelInfo";
import { TtsRunControl } from "@/components/tts/TtsRunControl";
import { TtsTestDialog } from "@/components/tts/TtsTestDialog";
import { TtsVoicesDialog } from "@/components/tts/TtsVoicesDialog";

/** 语音合成（TTS）配置页：标题行含启用开关与状态 + 基础配置 + 模型信息 + 高级参数。 */
export function TtsPage() {
  const [testOpen, setTestOpen] = useState(false);
  const [voicesOpen, setVoicesOpen] = useState(false);
  const [modelDialogOpen, setModelDialogOpen] = useState(false);

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
          语音合成（TTS）配置
        </h1>
        <TtsRunControl />
      </header>

      <TtsBasicConfig
        onTestOpen={() => setTestOpen(true)}
        onManageVoices={() => setVoicesOpen(true)}
        onOpenModelDialog={() => setModelDialogOpen(true)}
      />

      <TtsModelInfo />

      <TtsAdvancedParams />

      <TtsTestDialog
        open={testOpen}
        onClose={() => setTestOpen(false)}
        onManageVoices={() => setVoicesOpen(true)}
        manageOpen={voicesOpen}
      />

      <TtsVoicesDialog open={voicesOpen} onClose={() => setVoicesOpen(false)} />

      <TtsModelDialog open={modelDialogOpen} onClose={() => setModelDialogOpen(false)} />
    </div>
  );
}
