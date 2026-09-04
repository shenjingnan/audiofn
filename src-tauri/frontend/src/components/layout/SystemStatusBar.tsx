import { Cpu, HardDrive, MemoryStick } from "lucide-react";
import { useMemo } from "react";
import { useSystemResources } from "@/hooks/useSystemResources";

/** 字节 → 人类可读（GB/ TB，保留 1 位小数）。 */
function fmtBytes(bytes: number): string {
  if (bytes >= 1e12) return `${(bytes / 1e12).toFixed(1)} TB`;
  return `${(bytes / 1e9).toFixed(1)} GB`;
}

/** 根据使用率返回进度条颜色：正常品牌蓝，≥60% 琥珀预警，≥85% 红告警。 */
function usageColor(ratio: number): string {
  if (ratio < 0.6) return "bg-primary";
  if (ratio < 0.85) return "bg-amber-500";
  return "bg-red-500";
}

/** 单个指标：图标 + 标签 + 迷你进度条 + 数值。 */
function MetricChip({
  icon: Icon,
  label,
  value,
  sub,
  ratio,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  value: string;
  sub?: string;
  ratio: number;
}) {
  return (
    <div className="flex items-center gap-1.5 text-[11px] leading-none text-text-secondary select-none">
      <Icon className="h-3 w-3 shrink-0 text-text-muted" />
      <span className="text-text-muted">{label}</span>
      {/* 迷你进度条：按负载变色（蓝 → 琥珀 → 红），中性灰轨道 */}
      <div className="h-1 w-7 shrink-0 overflow-hidden rounded-full bg-black/10">
        <div
          className={`h-full rounded-full transition-all duration-500 ${usageColor(ratio)}`}
          style={{ width: `${Math.min(ratio * 100, 100)}%` }}
        />
      </div>
      <span className="tabular-nums text-text-primary">{value}</span>
      {sub && <span className="tabular-nums text-text-muted">{sub}</span>}
    </div>
  );
}

/** 顶部拖拽条中的系统状态栏：CPU / 内存 / 磁盘。
 *
 * 白色小卡片（同 MainPanel 的边框/轻阴影语言），左缘与内容卡片对齐；
 * 紧凑设计适配 36px 拖拽条高度，3 个指标横向排列。
 * 数据每 30s 自动刷新（页面不可见时暂停）。
 */
export function SystemStatusBar() {
  const { resources } = useSystemResources();

  const metrics = useMemo(() => {
    if (!resources) return null;

    const memUsed = resources.totalMemory - resources.availableMemory;
    const memRatio = resources.totalMemory > 0 ? memUsed / resources.totalMemory : 0;
    const diskUsed = resources.diskTotal - resources.diskAvailable;
    const diskRatio = resources.diskTotal > 0 ? diskUsed / resources.diskTotal : 0;
    const cpuRatio = resources.cpuUsage / 100;

    return {
      cpu: { value: `${resources.cpuUsage.toFixed(0)}%`, ratio: cpuRatio },
      mem: {
        value: fmtBytes(memUsed),
        sub: `/ ${fmtBytes(resources.totalMemory)}`,
        ratio: memRatio,
      },
      disk: {
        value: fmtBytes(diskUsed),
        sub: `/ ${fmtBytes(resources.diskTotal)}`,
        ratio: diskRatio,
      },
    };
  }, [resources]);

  if (!metrics) return null;

  return (
    <div className="flex items-center gap-1.5 rounded-md border border-panel-border bg-panel-background px-2.5 py-1 shadow-[0_1px_2px_rgba(15,23,42,0.03)]">
      <MetricChip icon={Cpu} label="CPU" {...metrics.cpu} />
      <MetricChip icon={MemoryStick} label="MEM" {...metrics.mem} />
      <MetricChip icon={HardDrive} label="DISK" {...metrics.disk} />
    </div>
  );
}
