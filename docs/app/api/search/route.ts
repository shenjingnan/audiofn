import { source } from '@/lib/source';
import { createFromSource } from 'fumadocs-core/search/server';

export const revalidate = false;

// 静态导出：构建期把搜索索引预渲染为 out/api/search
export const { staticGET: GET } = createFromSource(source);
