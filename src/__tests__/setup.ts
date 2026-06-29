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
