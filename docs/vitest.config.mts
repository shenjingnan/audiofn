import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    // downloads.ts 无 DOM，node 环境即可，无需 jsdom/vite 插件
    environment: 'node',
    include: ['lib/**/*.test.ts'],
  },
});
