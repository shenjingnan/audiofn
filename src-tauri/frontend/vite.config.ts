/// <reference types="vitest/config" />
import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  // Vite 开发服务器端口需与 tauri.conf.json 的 devUrl 一致
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  // 唯一入口：settings.html（设置面板）
  build: {
    rollupOptions: {
      input: {
        settings: path.resolve(import.meta.dirname, "settings.html"),
      },
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: "./vitest.setup.ts",
    css: false,
  },
});
