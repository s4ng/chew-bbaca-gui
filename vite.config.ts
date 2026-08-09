import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri는 고정 포트를 기대한다. 포트가 점유되어 있으면 조용히 다른 포트로
// 옮겨가지 않고 실패해야 한다 (셸이 빈 화면을 띄우는 것보다 낫다).
export default defineConfig(({ mode }) => ({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Rust 쪽 변경은 cargo가 감시한다.
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    // WebView2 는 최신 Chromium 이므로 다운레벨링이 필요 없다.
    target: "chrome105",
    sourcemap: mode === "development",
    minify: mode !== "development",
  },
}));
