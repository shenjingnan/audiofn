import type { Metadata } from 'next';
import { HomeLayout } from 'fumadocs-ui/layouts/home';
import { HomeFooter } from '@/components/home/home-footer';
import { HomeHero } from '@/components/home/home-hero';
import { LogoMark } from '@/components/home/logo-mark';
import { ScreenshotSection } from '@/components/home/screenshot-section';
import { baseOptions } from '@/lib/layout.shared';

export const metadata: Metadata = {
  title: 'AudioFn — 开源的实时桌面 AI 伙伴',
  description:
    '开源的实时桌面 AI 伙伴：语音唤醒、语音识别、本地大语言模型与 Live2D 虚拟角色于一体，本地优先，数据不出设备。',
};

export default function Page() {
  const options = baseOptions();
  return (
    <HomeLayout
      {...options}
      // 首页覆盖 nav.title 为品牌 Logo；nav.url 指向首页
      nav={{ ...options.nav, title: <LogoMark />, url: '/' }}
      // header：文档入口（新标签页打开）；GitHub 由右侧 githubUrl 图标提供
      links={[{ text: '文档', url: '/docs', external: true }]}
    >
      <HomeHero />
      <ScreenshotSection />
      <HomeFooter />
    </HomeLayout>
  );
}
