import type { PlatformKey } from '@/lib/downloads';

interface IconProps {
  className?: string;
}

/**
 * 平台品牌图标与通用图标（内联 SVG，lucide 风格 24×24 stroke）。
 * 不引入 lucide-react：docs 包拿不到 fumadocs-ui 的传递依赖，且平台
 * 品牌图标 lucide 本来也不提供。
 */
export function WindowsIcon({ className }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      <path d="M3 5.5 10.5 4v6.5H3Z" />
      <path d="M14.5 3.5 21 4.5V11h-6.5Z" />
      <path d="M3 13h7.5V20L3 18.5Z" />
      <path d="M14.5 13H21v7l-6.5-1.5Z" />
    </svg>
  );
}

export function AppleIcon({ className }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      <path d="M12 20.94c1.5 0 2.75 1.06 4 1.06 3 0 6-8 6-12.22A4.91 4.91 0 0 0 17 5c-2.22 0-4 1.44-5 2-1-.56-2.78-2-5-2a4.9 4.9 0 0 0-5 4.78C2 14 5 22 8 22c1.25 0 2.5-1.06 4-1.06Z" />
      <path d="M10 2c1 .5 2 2 2 5" />
    </svg>
  );
}

export function LinuxIcon({ className }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      <path d="M12 2.5a5 5 0 0 0-4.23 7.5c-.6.33-1.27.86-1.27 2a6 6 0 0 0-2.5 4.5c0 2 1 3 2 4a10.5 10.5 0 0 0 3.5 2c0 1 0 2 1 2.5s2 .5 2 .5 1 0 2-.5 1-1.5 1-2.5c1.5-.5 2.5-1 3.5-2s2-2 2-4a6 6 0 0 0-2.5-4.5c0-1.14-.67-1.67-1.27-2A5 5 0 0 0 12 2.5Z" />
    </svg>
  );
}

export function DownloadIcon({ className }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
      <path d="m7 10 5 5 5-5" />
      <path d="M12 15V3" />
    </svg>
  );
}

export function ChevronDownIcon({ className }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      <path d="m6 9 6 6 6-6" />
    </svg>
  );
}

/** 根据平台 key 渲染对应品牌图标；unknown/未检测时用下载图标兜底。 */
export function PlatformIcon({
  platform,
  className,
}: {
  platform: PlatformKey;
  className?: string;
}) {
  switch (platform) {
    case 'windows-x64':
      return <WindowsIcon className={className} />;
    case 'macos-arm64':
    case 'macos-x64':
      return <AppleIcon className={className} />;
    case 'linux-x64':
      return <LinuxIcon className={className} />;
    default:
      return <DownloadIcon className={className} />;
  }
}
