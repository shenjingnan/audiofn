import { AudioLines, ChevronDown } from "lucide-react";
import { useState } from "react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";

/**
 * 模型信息（默认展开）：运行时 / 执行 Provider / 线程数 / 支持语言 / 模型目录 / 配置路径。
 * 全部来自 get_tts_config 的只读字段，无任何可编辑项。
 * 注意：config.backend（audiocpp）是引擎运行时，config.provider（cpu/metal）是执行
 * Provider，两者概念不同。
 * 采样率不在 get_tts_config 中（仅在合成结果 tts-result.sample_rate 出现），故此处不展示。
 */
export function TtsModelInfo() {
  const { tts } = useRuntime();
  const [open, setOpen] = useState(true);
  const config = tts.config;

  return (
    <section className="rounded-[16px] border border-panel-border bg-panel-background">
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger className="flex items-center justify-between gap-2 px-4 py-3 text-left">
          <span className="flex items-center gap-2.5">
            <AudioLines className="h-4 w-4 shrink-0 text-text-secondary" />
            <span>
              <h2 className="text-base font-semibold text-text-primary">模型信息</h2>
              <p className="mt-0.5 text-xs text-text-muted">运行时、执行 Provider、模型目录等</p>
            </span>
          </span>
          <ChevronDown
            className={cn(
              "h-4 w-4 shrink-0 text-text-muted transition-transform",
              open && "rotate-180",
            )}
          />
        </CollapsibleTrigger>
        <CollapsibleContent className="border-t border-divider">
          {config && (
            <dl>
              {/* 运行时来自 config.backend（一期 TTS 只有 audiocpp sidecar） */}
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">运行时</dt>
                <dd className="truncate text-sm text-text-secondary">
                  {config.backend === "audiocpp" ? "audio.cpp" : config.backend}
                </dd>
              </div>
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">执行 Provider</dt>
                <dd className="truncate text-sm text-text-secondary">{config.provider}</dd>
              </div>
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">线程数</dt>
                <dd className="truncate text-sm text-text-secondary">{config.num_threads}</dd>
              </div>
              {/* 固定模型：zipvoice-distill-zh-en 为中英双语，无语言选择 */}
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">支持语言</dt>
                <dd className="truncate text-sm text-text-secondary">中文、English</dd>
              </div>
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">模型目录</dt>
                <dd className="truncate text-sm text-text-secondary">{config.model_dir}</dd>
              </div>
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">配置路径</dt>
                <dd className="truncate text-sm text-text-secondary">{config.settings_path}</dd>
              </div>
            </dl>
          )}
        </CollapsibleContent>
      </Collapsible>
    </section>
  );
}
