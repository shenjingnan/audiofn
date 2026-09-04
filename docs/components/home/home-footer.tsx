import { RELEASES_PAGE } from '@/lib/downloads';

/** 首页页脚：版权 + 常用链接。 */
export function HomeFooter() {
  return (
    <footer className="mt-24 border-t border-fd-border">
      <div className="mx-auto flex max-w-7xl flex-col items-center justify-between gap-4 px-6 py-8 sm:flex-row">
        <p className="text-sm text-fd-muted-foreground">
          © {new Date().getFullYear()} AudioFn · GPL-3.0
        </p>
        <nav className="flex flex-wrap justify-center gap-x-6 gap-y-2 text-sm text-fd-muted-foreground">
          <a
            href="https://github.com/shenjingnan/audiofn"
            className="transition-colors hover:text-fd-foreground"
          >
            GitHub
          </a>
          <a
            href="/docs"
            className="transition-colors hover:text-fd-foreground"
          >
            文档
          </a>
          <a
            href={RELEASES_PAGE}
            className="transition-colors hover:text-fd-foreground"
          >
            Releases
          </a>
          <a
            href="/docs/license"
            className="transition-colors hover:text-fd-foreground"
          >
            许可证
          </a>
        </nav>
      </div>
    </footer>
  );
}
