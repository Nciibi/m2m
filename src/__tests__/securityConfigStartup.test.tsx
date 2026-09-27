import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

/**
 * Security-config startup behaviour.
 *
 * These are the tests for a bug where the app was *silently defenceless* on
 * every launch. `get_security_config` was fetched only from inside
 * `openSettings()`, which runs on a gear-button click. Until the user happened
 * to open Settings, `securityConfig` was `null`, and because every consumer
 * fails closed on null (`?? false`, `?? 0`) that meant:
 *
 *   - the panic-wipe hotkey was never bound at all,
 *   - the idle auto-lock timeout was 0 (disabled),
 *   - focus-loss blur was off.
 *
 * The user arms panic wipe, quits, reopens the app in an emergency, presses
 * Ctrl+Alt+Shift+W — and nothing happens. Worse, `App.tsx` calls
 * `reapply_security_config` on mount so the *backend* protections did survive,
 * which meant the setting still rendered as "enabled" in Settings while the
 * frontend behaviour was dead.
 *
 * `App.tsx` was at 0% coverage and the Settings view tests mock `useSettings`
 * wholesale, which is exactly why this survived.
 */

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: any[]) => mockInvoke(...a) }));

const appState = {
  addToast: vi.fn(),
  setView: vi.fn(),
  view: "hub" as string,
  identity: null,
  vaultInitialized: true,
  vaultUnlocked: true,
  refreshVault: vi.fn(),
  toasts: [],
  removeToast: vi.fn(),
};
vi.mock("../context/AppContext", () => ({
  useApp: () => appState,
  AppProvider: ({ children }: any) => children,
}));

const securityState = {
  securityConfig: null as any,
  scheduleClipboardClear: vi.fn(),
};
vi.mock("../context/SettingsContext", () => ({
  useSettings: () => securityState,
}));

vi.mock("../context/ChatContext", () => ({
  useChat: () => ({ handleDisconnect: vi.fn(), connection: null, messages: [], setMessages: vi.fn() }),
}));

vi.mock("../components/Sidebar", () => ({ default: () => null }));
vi.mock("../components/ShortcutHelp", () => ({ default: () => null }));
vi.mock("../components/FamilyTab", () => ({ default: () => null }));
vi.mock("../components/chat/MessageBubble", () => ({ default: () => null }));
vi.mock("../components/ErrorBoundary", () => ({ default: ({ children }: any) => children }));
vi.mock("../views/SetupView", () => ({ default: () => null }));
vi.mock("../views/HubView", () => ({ default: () => null }));
vi.mock("../views/ChatView", () => ({ default: () => null }));
vi.mock("../views/SettingsView", () => ({ default: () => null }));
vi.mock("../views/VaultView", () => ({ default: () => null }));
vi.mock("../views/GroupChatView", () => ({ default: () => null }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));

import { SettingsProvider, useSettings } from "../context/SettingsContext";

const FULL_CONFIG = {
  screen_capture_protection: false,
  clipboard_clear_secs: 0,
  idle_lock_secs: 0,
  require_known_contact: false,
  capture_process_detection: false,
  blur_on_focus_loss: false,
  air_gap_mode: false,
  ephemeral_mode: false,
  send_batching_ms: 0,
  cover_typing_traffic: false,
  panic_hotkey_enabled: false,
};

function ConfigProbe() {
  const { securityConfig } = useSettings();
  return (
    <div>
      <span data-testid="loaded">{String(securityConfig !== null)}</span>
      <span data-testid="panic">{String(securityConfig?.panic_hotkey_enabled ?? "unset")}</span>
      <span data-testid="idle">{String(securityConfig?.idle_lock_secs ?? "unset")}</span>
      <span data-testid="blur">{String(securityConfig?.blur_on_focus_loss ?? "unset")}</span>
    </div>
  );
}

describe("security config loads on mount", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_security_config") return { ...FULL_CONFIG };
      return undefined;
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("fetches the persisted config on mount, without opening Settings", async () => {
    render(
      <SettingsProvider>
        <ConfigProbe />
      </SettingsProvider>
    );

    // The whole point: the read happens on mount. No user interaction, no
    // `setView("settings")`, no gear-button click.
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("get_security_config"),
    );
    await waitFor(() =>
      expect(screen.getByTestId("loaded")).toHaveTextContent("true"),
    );
    // And `openSettings` was never invoked.
    expect(appState.setView).not.toHaveBeenCalled();
  });

  it("exposes the armed panic-wipe setting so the hotkey can bind", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_security_config") {
        return { ...FULL_CONFIG, panic_hotkey_enabled: true };
      }
      return undefined;
    });

    render(
      <SettingsProvider>
        <ConfigProbe />
      </SettingsProvider>
    );

    // `App.tsx` gates the hotkey listener on this exact value. While it read
    // "unset", the keydown handler was never attached.
    await waitFor(() =>
      expect(screen.getByTestId("panic")).toHaveTextContent("true"),
    );
  });

  it("exposes the idle-lock timeout so auto-lock is not silently disabled", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_security_config") {
        return { ...FULL_CONFIG, idle_lock_secs: 300 };
      }
      return undefined;
    });

    render(
      <SettingsProvider>
        <ConfigProbe />
      </SettingsProvider>
    );

    // `App.tsx` passes `securityConfig?.idle_lock_secs ?? 0` to
    // useIdleDetection. 0 disables it.
    await waitFor(() =>
      expect(screen.getByTestId("idle")).toHaveTextContent("300"),
    );
  });

  it("exposes the focus-loss blur setting", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_security_config") {
        return { ...FULL_CONFIG, blur_on_focus_loss: true };
      }
      return undefined;
    });

    render(
      <SettingsProvider>
        <ConfigProbe />
      </SettingsProvider>
    );

    await waitFor(() =>
      expect(screen.getByTestId("blur")).toHaveTextContent("true"),
    );
  });

  it("warns the user when the config cannot be read", async () => {
    // Failing open silently is the other half of the original bug: the user
    // would believe their protections were active.
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_security_config") throw new Error("vault locked");
      return undefined;
    });

    render(
      <SettingsProvider>
        <ConfigProbe />
      </SettingsProvider>
    );

    await waitFor(() => expect(appState.addToast).toHaveBeenCalled());
    const [msg, kind] = appState.addToast.mock.calls[0];
    expect(String(msg)).toMatch(/security settings/i);
    expect(String(msg)).toMatch(/inactive/i);
    expect(kind).toBe("error");
  });

  it("fails closed when the config read fails", async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_security_config") throw new Error("boom");
      return undefined;
    });

    render(
      <SettingsProvider>
        <ConfigProbe />
      </SettingsProvider>
    );

    // Stays null so every consumer's `?? false` / `?? 0` fallback applies.
    // The point is that it does NOT silently become a permissive object.
    expect(screen.getByTestId("loaded")).toHaveTextContent("false");
    expect(screen.getByTestId("panic")).toHaveTextContent("unset");
  });
});
