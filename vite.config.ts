import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri는 고정 포트를 기대한다. 포트가 점유되어 있으면 조용히 다른 포트로
// 옮겨가지 않고 실패해야 한다 (셸이 빈 화면을 띄우는 것보다 낫다).
//
// **이 값을 바꾸면 `src-tauri/tauri.conf.json` 의 `devUrl` 도 같이 바꾼다.**
// 한쪽만 고치면 Vite 는 정상 기동하고 앱만 빈 화면을 띄운다.
//
// 1420(Tauri 기본값)에서 옮긴 이유: Windows 의 Hyper-V/WSL2 가 부팅할 때마다
// 포트 대역을 동적으로 예약하는데, 그 대역에 1420 이 걸리면 Vite 가
// `EACCES ::1:1420` 으로 죽는다. 점유가 아니라 예약이라 프로세스를 찾아도 없다.
// 확인: `netsh interface ipv4 show excludedportrange protocol=tcp`
export default defineConfig(({ mode }) => ({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
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
