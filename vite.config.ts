/// <reference types="vitest" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },

  // ── Vitest Configuration ──
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/__tests__/setup.ts"],
    css: true,
    coverage: {
      provider: "v8",
      reporter: ["text", "lcov"],
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/__tests__/**",
        "src/main.tsx",
        "src/components/ui/icons/**"
      ],
// Coverage floors.
      //
      // These were 45/55/30/45 — where coverage actually sat at the time, so they
      // could never fail and enforced nothing. Then they were raised to
      // 50/60/34/50, set just under numbers that were themselves stale: the real
      // figures are 65.42 / 76.57 / 55.72 / 65.42, leaving `functions` only
      // 1.8 points of headroom, so one new uncovered function broke CI.
      //
      // Raised again to sit comfortably under the measured values, which is what
      // a floor is for: fail on a regression, not on ordinary churn.
      //
      // `functions` stays the lowest because that is the honest shape — a
      // function counts as covered the moment it runs once, and a large share of
      // this UI is render-only. Raising it further would be theatre.
      thresholds: {
        statements: 60,
        branches: 70,
        functions: 48,
        lines: 60
      }
    }
  },
}));
