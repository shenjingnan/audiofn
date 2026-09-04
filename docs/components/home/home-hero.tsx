import { RELEASES_PAGE } from '@/lib/downloads';
import { DownloadSection } from './download-section';

/** 渐变 blob 的多边形（Tailwind 无 clip-path 工具类，必须内联 style）。 */
const BLOB_POLYGON =
  'polygon(74.1% 44.1%, 100% 61.6%, 97.5% 26.9%, 85.5% 0.1%, 80.7% 2%, 72.5% 32.5%, 60.2% 62.4%, 52.4% 68.1%, 47.5% 58.3%, 45.2% 34.5%, 27.5% 76.7%, 0.1% 64.9%, 17.9% 100%, 27.6% 76.8%, 76.1% 97.7%, 74.1% 44.1%)';

/** 顶部与底部模糊渐变 blob 的公共 class（dark 下调低透明度）。 */
const BLOB_CLASS =
  'bg-linear-to-tr from-[#ff80b5] to-[#9089fc] opacity-30 dark:from-[#1a7df8]/40 dark:to-[#c026d3]/20';

/**
 * 首页 hero：居中标题 + 一句话卖点 + 下载区 + 上下渐变 blob 背景。
 * 参考 Tailwind UI「Simple centered」结构，配色改用 Fumadocs 语义色。
 */
export function HomeHero() {
  return (
    <section className="relative isolate overflow-hidden px-6 pt-14 lg:px-8">
      <div
        aria-hidden="true"
        className="absolute inset-x-0 top-0 -z-10 transform-gpu overflow-hidden blur-3xl"
      >
        <div
          style={{ clipPath: BLOB_POLYGON }}
          className={`relative left-1/2 aspect-1155/678 w-[36.125rem] -translate-x-1/2 rotate-[30deg] sm:w-[72.1875rem] ${BLOB_CLASS}`}
        />
      </div>
      <div
        aria-hidden="true"
        className="absolute inset-x-0 top-[calc(100%-13rem)] -z-10 transform-gpu overflow-hidden blur-3xl sm:top-[calc(100%-30rem)]"
      >
        <div
          style={{ clipPath: BLOB_POLYGON }}
          className={`relative left-[calc(50%+3rem)] aspect-1155/678 w-[36.125rem] -translate-x-1/2 sm:left-[calc(50%+36rem)] sm:w-[72.1875rem] ${BLOB_CLASS}`}
        />
      </div>

      <div className="mx-auto max-w-2xl py-24 text-center sm:py-32 lg:py-40">
        <div className="hidden sm:mb-8 sm:flex sm:justify-center">
          <div className="inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-sm/6 text-fd-muted-foreground ring-1 ring-fd-border">
            <span className="size-1.5 rounded-full bg-fd-primary" />
            开源 · 免费 · 本地优先
            <a
              href={RELEASES_PAGE}
              className="font-semibold text-fd-primary hover:underline"
            >
              查看版本 <span aria-hidden="true">→</span>
            </a>
          </div>
        </div>

        <h1 className="text-balance text-4xl font-semibold tracking-tight text-fd-foreground sm:text-6xl">
          开源的本地优先桌面 ASR &amp; TTS 工具
        </h1>

        <p className="mt-6 text-pretty text-lg/8 text-fd-muted-foreground">
          Qwen3-ASR 离线转写，30 语言自动识别；Qwen3-TTS 用 5–30
          秒参考音频克隆你的音色。桌面与 CLI 双形态，全部在本地运行，数据不出设备；macOS
          / Linux 可用。
        </p>

        <DownloadSection />
      </div>
    </section>
  );
}
