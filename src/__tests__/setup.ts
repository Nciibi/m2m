import "@testing-library/jest-dom";
import { vi } from "vitest";

// JSDOM doesn't implement window.matchMedia; provide a stub.
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  }),
});

// Mock @tauri-apps/plugin-notification globally so it doesn't throw in jsdom
vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(false),
  sendNotification: vi.fn(),
  requestPermission: vi.fn().mockResolvedValue("granted"),
}));

/**
 * Every component that renders user-facing text now calls `useT()`, so tests
 * that mount a provider need `I18nProvider` in scope. Rather than repeat the
 * wrapper in each of the dozen test files, export a `render` that includes it.
 *
 * Uses `createElement` rather than JSX because this file is `.ts` — renaming it
 * to `.tsx` would mean touching the vitest config for no benefit.
 *
 * Tests needing a specific locale can nest another `I18nProvider`; the English
 * default is what almost every assertion expects.
 */
import { render as rtlRender, RenderOptions, RenderResult } from "@testing-library/react";
import { createElement, ReactElement, ReactNode } from "react";
import { I18nProvider } from "../i18n/I18nContext";

function Providers({ children }: { children: ReactNode }) {
  return createElement(I18nProvider, null, children);
}

export function render(
  ui: ReactElement,
  options?: Omit<RenderOptions, "wrapper">,
): RenderResult {
  return rtlRender(ui, { wrapper: Providers, ...options });
}
