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
      // These were set to 45/55/30/45 — which is where coverage actually sat,
      // so they could never fail and enforced nothing. Tightened to sit just
      // under the current real numbers (52.1 / 64.7 / 35.8 / 52.1) so that a
      // regression fails the build but a normal edit does not.
      //
      // `functions` stays low because that is the honest number: a function
      // counts as covered as soon as it runs once, and a meaningful share of
      // the UI is render-only. Raising it further would be theatre.
      thresholds: {
        statements: 50,
        branches: 60,
        functions: 34,
        lines: 50
      }
    }
  },
}));
