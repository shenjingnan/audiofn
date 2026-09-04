import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

/** 加载骨架（shadcn 风格 animate-pulse）。 */
function Skeleton({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn("animate-pulse rounded-md bg-muted", className)} {...props} />;
}

/** 模型卡片骨架。 */
export function ModelCardSkeleton() {
  return (
    <div className="flex flex-col gap-3 rounded-[16px] border border-panel-border bg-panel-background px-5 py-4">
      <div className="flex items-center gap-3">
        <Skeleton className="h-9 w-9 rounded-full" />
        <div className="flex-1 space-y-2">
          <Skeleton className="h-4 w-48" />
          <Skeleton className="h-3 w-72" />
        </div>
        <Skeleton className="h-9 w-20 rounded-md" />
      </div>
      <div className="flex items-center gap-2">
        <Skeleton className="h-5 w-16 rounded-full" />
        <Skeleton className="h-5 w-16 rounded-full" />
        <Skeleton className="h-5 w-16 rounded-full" />
      </div>
      <Skeleton className="h-3 w-40" />
    </div>
  );
}

export { Skeleton };
