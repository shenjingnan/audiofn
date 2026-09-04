import { ChevronDown, Cpu } from "lucide-react";
import { useState } from "react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";
import { useRuntime } from "@/providers/RuntimeContext";

/**
 * 模型信息（默认折叠）：推理后端 / 采样率 / 内置关键词。
 * 全部来自 get_kws_config 的只读字段，无任何可编辑项。
 */
export function KwsTechnicalInfo() {
  const { kws } = useRuntime();
  const [open, setOpen] = useState(false);
  const config = kws.config.config;

  return (
    <section className="rounded-[16px] border border-panel-border bg-panel-background">
      <Collapsible open={open} onOpenChange={setOpen}>
        <CollapsibleTrigger className="flex items-center justify-between gap-2 px-4 py-3 text-left">
          <span className="flex items-center gap-2.5">
            <Cpu className="h-4 w-4 shrink-0 text-text-secondary" />
            <span>
              <h2 className="text-base font-semibold text-text-primary">模型信息</h2>
              <p className="mt-0.5 text-xs text-text-muted">采样率、内置关键词、推理后端等</p>
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
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">推理后端</dt>
                <dd className="truncate text-sm text-text-secondary">{config.provider}</dd>
              </div>
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">采样率</dt>
                <dd className="truncate text-sm text-text-secondary">{config.sample_rate}</dd>
              </div>
              <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
                <dt className="text-sm text-text-primary">内置关键词</dt>
                <dd className="truncate text-sm text-text-secondary">
                  {config.keywords.join("、") || "（空）"}
                </dd>
              </div>
            </dl>
          )}
        </CollapsibleContent>
      </Collapsible>
    </section>
  );
}
