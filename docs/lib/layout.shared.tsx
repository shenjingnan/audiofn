import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: 'ZapMomo 文档',
    },
    githubUrl: 'https://github.com/shenjingnan/zapmomo',
    links: [
      {
        text: 'GitHub',
        url: 'https://github.com/shenjingnan/zapmomo',
      },
    ],
  };
}
