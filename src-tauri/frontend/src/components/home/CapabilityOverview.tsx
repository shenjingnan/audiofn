import { type CapabilityStatus, OVERVIEW_STATUS_COLOR } from "@/components/home/overviewMeta";
import { cn } from "@/lib/utils";

/**
 * 概览页「AI 能力」卡片：语音识别 / 语音合成（一行两卡，随高度伸展）。
 *
 * 纯状态展示（Icon + 名称 + 缩写 + 状态点），不做点击导航。
 */
export function CapabilityOverview({ statuses }: { statuses: CapabilityStatus[] }) {
  return (
    <section
      aria-label="AI 能力"
      className="flex min-h-0 flex-col rounded-[16px] border border-panel-border bg-panel-background"
    >
      <div className="px-5 py-4">
        <h2 className="text-base font-semibold text-text-primary">AI 能力</h2>
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-2 gap-3 px-5 pb-5">
        {statuses.map((status) => (
          <CapabilityCard key={status.key} status={status} />
        ))}
      </div>
    </section>
  );
}

function CapabilityCard({ status }: { status: CapabilityStatus }) {
  const Icon = status.icon;
  return (
    <div className="flex items-center gap-3 rounded-xl border border-panel-border px-3.5 py-3">
      <span
        className={cn(
          "flex h-9 w-9 shrink-0 items-center justify-center rounded-full",
          status.accent,
        )}
      >
        <Icon className="h-4 w-4" />
      </span>
      <div className="min-w-0">
        <p className="flex items-baseline gap-1.5 text-sm font-medium text-text-primary">
          {status.name}
          <span className="text-[11px] font-normal text-text-muted">{status.code}</span>
        </p>
        <span
          className={cn(
            "mt-0.5 flex items-center gap-1.5 text-xs",
            OVERVIEW_STATUS_COLOR[status.tone],
          )}
        >
          <span className="h-1.5 w-1.5 rounded-full bg-current" />
          {status.label}
        </span>
      </div>
    </div>
  );
}
