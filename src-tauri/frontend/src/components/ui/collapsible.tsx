import type { ComponentProps, ReactNode } from "react";
import { createContext, useCallback, useContext } from "react";
import { cn } from "@/lib/utils";

const CollapsibleContext = createContext<{ open: boolean; toggle: () => void }>({
  open: false,
  toggle: () => {},
});

interface CollapsibleProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
  className?: string;
}

/**
 * 轻量可折叠区块，无第三方依赖。
 * 折叠用 `grid-template-rows` 过渡实现纯 CSS 高度动画，内容常驻 DOM（受控 Switch 状态不丢失）。
 */
function Collapsible({ open, onOpenChange, children, className }: CollapsibleProps) {
  const toggle = useCallback(() => onOpenChange(!open), [open, onOpenChange]);
  return (
    <CollapsibleContext.Provider value={{ open, toggle }}>
      <div className={className}>{children}</div>
    </CollapsibleContext.Provider>
  );
}

function CollapsibleTrigger({ className, ...props }: ComponentProps<"button">) {
  const { open, toggle } = useContext(CollapsibleContext);
  return (
    <button
      type="button"
      aria-expanded={open}
      onClick={toggle}
      className={cn("w-full", className)}
      {...props}
    />
  );
}

function CollapsibleContent({ className, children, ...props }: ComponentProps<"div">) {
  const { open } = useContext(CollapsibleContext);
  return (
    <div
      className={cn(
        "grid transition-[grid-template-rows] duration-200 ease-out",
        open ? "grid-rows-[1fr]" : "grid-rows-[0fr]",
      )}
      {...props}
    >
      <div className="overflow-hidden">
        <div className={className}>{children}</div>
      </div>
    </div>
  );
}

export { Collapsible, CollapsibleContent, CollapsibleTrigger };
