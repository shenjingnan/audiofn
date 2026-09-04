import { FolderOpen } from "lucide-react";
import { useState } from "react";
import { AsrModelDialog } from "@/components/asr/AsrModelDialog";
import { Button } from "@/components/ui/button";
import { useRuntime } from "@/providers/RuntimeContext";

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/**
 * 摘要 ASR 行的模型快速切换：模型名文本 + 「选择模型」按钮（与 TTS 行
 * 同款样式/文案/交互），打开同一个选择识别模型弹窗。
 * 组件自带点击冒泡拦截（按钮/弹窗内点击均不触发所在行的 Link 导航）。
 */
export function AsrModelSwitchMenu() {
  const { asr } = useRuntime();
  const [open, setOpen] = useState(false);

  return (
    // 拦截点击：stopPropagation 挡 react-router 的 JS 导航，preventDefault 取消浏览器
    // 「激活祖先 <a>」的原生默认行为（a 内嵌 button 时点击会跟随 href 整页跳转）。
    // biome-ignore lint/a11y/noStaticElementInteractions: 静态容器仅拦截鼠标冒泡，交互由内部按钮承载
    // biome-ignore lint/a11y/useKeyWithClickEvents: 仅拦截点击冒泡防误触导航，键盘交互由内部按钮处理
    <span
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
      }}
    >
      <span className="inline-flex min-w-0 items-center gap-1.5">
        <span className="truncate text-xs text-text-secondary">
          {basename(asr.config?.config?.model_dir ?? "")}
        </span>
        <Button size="sm" onClick={() => setOpen(true)} aria-label="选择识别模型">
          <FolderOpen className="h-4 w-4" />
          选择模型
        </Button>
      </span>
      <AsrModelDialog open={open} onClose={() => setOpen(false)} />
    </span>
  );
}
