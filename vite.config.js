import { defineConfig } from "vitest/config";
import { sveltekit } from "@sveltejs/kit/vite";
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;
// MOCK=1: run the UI in a plain browser against an in-memory fake backend (dev/screenshots only).
/** @type {Record<string, string>} */
const mock = process.env.MOCK
  ? {
      "@tauri-apps/api/core": new URL("./src/lib/mock/tauri.ts", import.meta.url).pathname,
      "@tauri-apps/api/event": new URL("./src/lib/mock/tauri.ts", import.meta.url).pathname,
    }
  : {};

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [sveltekit()],
  resolve: { alias: mock },
  test: { include: ["src/**/*.test.ts"] },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
