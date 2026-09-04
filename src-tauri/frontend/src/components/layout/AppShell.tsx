import { isMacOs, isWindows } from "@/lib/platform";
import { cn } from "@/lib/utils";
import { MainPanel } from "./MainPanel";
import { Sidebar } from "./Sidebar";
import { SystemStatusBar } from "./SystemStatusBar";
import { WindowControls } from "./WindowControls";

/** 统一 App Shell：无标题栏。
 * 三平台顶部透明拖拽条拉通全宽（整条可拖动窗口）：Linux/Windows 三键靠右；
 * macOS 无三键（系统红绿灯为原生控件，层级在 webview 之上不受影响；
 * Overlay 标题栏下原生拖拽区被 webview 覆盖，拖拽须由 HTML 拖拽区承担）。 */
export function AppShell() {
  const mac = isMacOs();
  const windows = isWindows();

  return (
    <div
      className={cn(
        "relative flex h-screen overflow-hidden bg-app-background text-foreground",
        // 圆角：macOS 由系统绘制；Linux 透明窗口需 CSS 圆角裁出；Windows 不透明方角无需处理。
        !mac && !windows && "rounded-xl",
        // Windows：后端已关 DWM shadow（undecorated+shadow 在 Win10 会被 DWM 画成
        // 左右底三边黑框、顶部强制无边），四边完整边框由 CSS 自绘。
        windows && "border border-window-border",
      )}
    >
      {/* 三平台：透明悬浮顶部条拉通全宽（整条可拖拽窗口）。
          状态栏（CPU/内存/磁盘）与白色内容卡片对齐；
          右侧：窗口三键（仅非 macOS）。
          macOS 红绿灯（78px）由系统原生绘制，左侧留白避让。 */}
      <div
        data-tauri-drag-region
        className="absolute left-0 right-0 top-0 z-10 flex h-9 items-center justify-between"
      >
        {/* 状态栏与 MainPanel 白色卡片左缘对齐：侧栏 248px + 面板左距 pl-1 = 252px。
            macOS 红绿灯（78px）位于侧栏上方，状态栏已在 252px 之外无需避让。 */}
        <div className="ml-[252px]">
          <SystemStatusBar />
        </div>
        {!mac && <WindowControls />}
      </div>
      <Sidebar />
      <MainPanel />
    </div>
  );
}
