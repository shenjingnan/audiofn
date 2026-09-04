/**
 * 应用截图展示区：假浏览器窗口框内嵌 home.png。
 * 对应 Tailwind UI「bordered app screenshot」变体。
 */
export function ScreenshotSection() {
  return (
    <div className="mx-auto mt-16 max-w-5xl px-6 sm:mt-24 lg:px-8">
      <div className="relative overflow-hidden rounded-2xl bg-fd-card shadow-2xl ring-1 ring-fd-border">
        <img
          src="/screenshots/home.png"
          alt="AudioFn 桌面应用概览页"
          width={1600}
          height={1030}
          loading="lazy"
          className="h-auto w-full"
        />
      </div>
      <p className="mt-4 text-center text-sm text-fd-muted-foreground">
        桌面应用「概览」页：展示当前伙伴与 AI 能力状态
      </p>
    </div>
  );
}
