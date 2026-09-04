import { Outlet } from "react-router-dom";

/** 右侧主内容面板：白色大圆角 + 轻边框 + 轻阴影，内嵌路由出口。 */
export function MainPanel() {
  return (
    // 三平台：顶部让出全宽悬浮拖拽条的高度，避免页面顶部内容被拖拽区覆盖。
    <div className="min-w-0 flex-1 p-2 pl-1 pt-9">
      <div className="h-full overflow-hidden rounded-[12px] border border-panel-border bg-panel-background shadow-[0_1px_2px_rgba(15,23,42,0.02),0_4px_20px_rgba(15,23,42,0.03)]">
        {/* 双层：滚动容器不带 padding（子元素 h-full 的百分比为整数基数），
            padding 移到内层满高容器，杜绝 100%+padding / fr 小数产生亚像素溢出 */}
        <div className="h-full overflow-y-auto">
          <div className="flex h-full flex-col p-3">
            <Outlet />
          </div>
        </div>
      </div>
    </div>
  );
}
