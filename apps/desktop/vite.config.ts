import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: "0.0.0.0",
    // 避开 Vite 默认的 5173：本机其他项目常年占着它
    port: 5273,
    strictPort: true,
    allowedHosts: true,
  },
  build: {
    outDir: "dist",
    // 按目标平台给语法基线：Windows 跑 WebView2（Chromium），没必要降级到 Safari 15
    target: process.platform === "win32" ? "chrome110" : "safari15",
    sourcemap: false,
    rollupOptions: {
      output: {
        manualChunks: {
          graph: ["@xyflow/react"],
          motion: ["framer-motion"],
        },
      },
    },
  },
});
