import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: 'AudioFn 文档',
    },
    githubUrl: 'https://github.com/shenjingnan/audiofn',
    links: [
      {
        text: 'GitHub',
        url: 'https://github.com/shenjingnan/audiofn',
      },
    ],
  };
}
