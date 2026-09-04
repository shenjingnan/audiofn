import { Badge } from "@/components/ui/badge";
import { useRuntime } from "@/providers/RuntimeContext";

/** 全局监听状态徽标：KWS 或 ASR 任一监听中显示「监听中」，否则「空闲」。 */
export function ListeningStatusBadge() {
  const { anyListening } = useRuntime();

  return (
    <Badge
      className={
        anyListening ? "bg-emerald-100 text-emerald-700" : "bg-muted text-muted-foreground"
      }
    >
      {anyListening ? "监听中" : "空闲"}
    </Badge>
  );
}
