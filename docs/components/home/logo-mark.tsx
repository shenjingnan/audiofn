/**
 * 导航栏品牌标识：Logo 图 + 文字，用作 HomeLayout 的 nav.title。
 * HomeLayout 会把它包进 <Link href={nav.url}>，无需自带链接。
 */
export function LogoMark() {
  return (
    <span className="inline-flex items-center gap-2">
      <img
        src="/favicon.svg"
        alt="ZapMomo"
        width={32}
        height={32}
        className="h-6 w-auto"
      />
      <span className="text-sm font-semibold">ZapMomo</span>
    </span>
  );
}
