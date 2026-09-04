'use client';

import type { ReactNode } from 'react';
import { RootProvider } from 'fumadocs-ui/provider/next';
import SearchDialog from '@/components/search';

export function Provider({ children }: { children: ReactNode }) {
  // 静态托管下默认的 fetchClient 搜索不可用，必须注入静态搜索对话框
  return <RootProvider search={{ SearchDialog }}>{children}</RootProvider>;
}
