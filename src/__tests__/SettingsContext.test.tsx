import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import { render } from "./setup";
import userEvent from "@testing-library/user-event";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => mockInvoke(...args) }));

const appState = {
  addToast: vi.fn(),
  setView: vi.fn(),
};
vi.mock("../context/AppContext", () => ({
  useApp: () => appState,
}));

import { SettingsProvider, useSettings } from "../context/SettingsContext";

function TestConsumer() {
  const {
    publicIp, stunLoading, privateMode,
    discoveryConfig, discoveredPeers,
    securityConfig,
    handleStunDiscover, handleAddStunServer, handleRemoveStunServer,
    handleResetStunDefaults, handlePrivateModeToggle, handleConnectivityCheck,
    handleTorToggle, setStunServerInput,
    handleLanToggle, handleDhtToggle, handleRefreshDiscovery,
    handleScreenCaptureToggle, handleLockVault, handleClearClipboard,
    handleStorageCapChange, storageUsage,
    relayConfig, relayHost, relayPort, setRelayHost, setRelayPort,
    handleRelaySave, handleRelayClear,
  } = useSettings();
  return (
    <div>
      <span data-testid="public-ip">{publicIp || "null"}</span>
      <span data-testid="stun-loading">{String(stunLoading)}</span>
      <span data-testid="private-mode">{String(privateMode)}</span>
      <span data-testid="lan-enabled">{String(discoveryConfig?.lan_enabled ?? false)}</span>
      <span data-testid="dht-enabled">{String(discoveryConfig?.dht_enabled ?? false)}</span>
      <span data-testid="discovered-count">{discoveredPeers.length}</span>
      <span data-testid="screen-capture">{String(securityConfig?.screen_capture_protection ?? false)}</span>
      <button onClick={handleStunDiscover}>STUN Discover</button>
      <button onClick={handlePrivateModeToggle}>Toggle Private</button>
      <button onClick={handleTorToggle}>Toggle Tor</button>
      <button onClick={handleConnectivityCheck}>Check Connectivity</button>
      <button onClick={handleAddStunServer}>Add STUN Server</button>
      <button onClick={() => handleRemoveStunServer(0)}>Remove STUN 0</button>
      <button onClick={handleResetStunDefaults}>Reset STUN</button>
      <button onClick={() => setStunServerInput("test:3478")}>Set STUN Input</button>
      <button onClick={handleLanToggle}>Toggle LAN</button>
      <button onClick={handleDhtToggle}>Toggle DHT</button>
      <button onClick={handleRefreshDiscovery}>Refresh Discovery</button>
      <button onClick={handleScreenCaptureToggle}>Toggle Screen Capture</button>
      <button onClick={handleLockVault}>Lock Vault</button>
      <button onClick={handleClearClipboard}>Clear Clipboard</button>
      <span data-testid="storage-cap">{String(securityConfig?.storage_cap_bytes ?? "unset")}</span>
      <span data-testid="storage-usage">{storageUsage ? String(storageUsage.used_bytes) : "null"}</span>
      <span data-testid="storage-usage-cap">{storageUsage ? String(storageUsage.cap_bytes) : "null"}</span>
      <button onClick={() => void handleStorageCapChange(5 * 1024 ** 3)}>Set Cap 5GB</button>
    </div>
  );
}

/// Full default `SecurityConfig`, matching the Rust struct.
const DEFAULT_SECURITY_CONFIG = {
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
  // 0 = "use the backend default", which is the 10 GiB cap. See
  // `effective_storage_cap`.
  storage_cap_bytes: 0,
};

/**
 * Command-aware default mock.
 *
 * The suite previously queued responses positionally with
 * `mockResolvedValueOnce`. That is brittle in a way that has already bitten:
 * `SettingsProvider` now fetches the persisted security config on mount (a
 * security control, not a UI nicety — without it the panic-wipe hotkey, the
 * idle auto-lock and the focus-loss blur are all inert until the user happens
 * to open Settings), so the mount-time call consumed the first queued response
 * and every subsequent test got someone else's value.
 *
 * Keying the mock on the command name makes the suite independent of how many
 * reads the provider performs, and of the order it performs them in.
 */
/** Stand-in for the backend's relay config. `null` is the disabled state. */
let relayStore: { host: string; port: number; has_auth_token: boolean } | null = null;

function defaultInvoke(cmd: string, args?: Record<string, unknown>): unknown {
  switch (cmd) {
    case "get_security_config":
      return { ...DEFAULT_SECURITY_CONFIG };
    case "set_security_config":
      return { ...((args?.config as object | undefined) ?? DEFAULT_SECURITY_CONFIG) };
    case "get_discovery_config":
      return { lan_enabled: false, dht_enabled: false };
    case "set_discovery_config":
      return args?.config ?? { lan_enabled: false, dht_enabled: false };
    case "get_discovered_peers":
    case "get_muted_conversations":
    case "refresh_discovery":
      return [];
    case "get_storage_usage":
      return { used_bytes: 3_221_225_472, cap_bytes: 10_737_418_240 };
    case "get_network_diagnostics":
      return { nat_type: "Unknown", stun_servers: [], candidates: [], connectivity: null };
    case "get_stun_config":
      return { servers: [], private_mode: false };
    case "set_stun_servers":
      return undefined;
    case "get_capture_capability":
      return { supported: false, reason: "test" };
    case "is_duress_configured":
      return false;
    case "get_network_settings":
      return { public_ip: null };
    case "check_connectivity":
      // `reachable: null`, not `true`. The backend cannot measure inbound TCP
    // reachability from a local STUN probe — the STUN-mapped UDP port belongs
    // to a throwaway socket — so it always returns `None` here, and `true`
    // documented a state the wire cannot produce.
    return { reachable: null, stun_agreement: true, nat_type: "Full Cone", public_addr: null, host_addrs: [], behind_symmetric_nat: false };
    case "discover_public_ip":
      return "203.0.113.1";
    case "set_private_mode":
    case "lock_vault":
    case "clear_clipboard":
    case "set_tor_enabled":
      return undefined;
    // Relay. `get_relay_config` returns the *view* — note there is no
    // `auth_token` key anywhere in this fixture, which is the property the
    // Rust side enforces by not deriving Serialize on `RelayConfig`.
    case "get_relay_config":
      return relayStore;
    case "set_relay_config":
      relayStore =
        (args?.config as { host: string; port: number; has_auth_token: boolean } | null | undefined) ?? null;
      return undefined;
    default:
      return undefined;
  }
}

describe("SettingsContext", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    appState.addToast.mockClear();
    appState.setView.mockClear();
    relayStore = null;
    mockInvoke.mockImplementation(defaultInvoke);
  });

  it("provides default values", () => {
    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );
    expect(screen.getByTestId("public-ip").textContent).toBe("null");
    expect(screen.getByTestId("stun-loading").textContent).toBe("false");
    expect(screen.getByTestId("private-mode").textContent).toBe("false");
    expect(screen.getByTestId("lan-enabled").textContent).toBe("false");
    expect(screen.getByTestId("dht-enabled").textContent).toBe("false");
    expect(screen.getByTestId("discovered-count").textContent).toBe("0");
  });

  it("handleStunDiscover calls Tauri invoke", async () => {
    const user = userEvent.setup();

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("STUN Discover"));
    expect(mockInvoke).toHaveBeenCalledWith("discover_public_ip");
    expect(mockInvoke).toHaveBeenCalledWith("get_network_diagnostics");
  });

  it("handlePrivateModeToggle calls Tauri invoke", async () => {
    const user = userEvent.setup();

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Toggle Private"));
    expect(mockInvoke).toHaveBeenCalledWith("set_private_mode", expect.any(Object));
  });

  it("handleTorToggle requires networkSettings", async () => {
    const user = userEvent.setup();
    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Toggle Tor"));
    // `handleTorToggle` bails out when `networkSettings` is null. Assert the
    // specific command was never issued — not that `invoke` was never called,
    // because the provider legitimately reads the security config on mount.
    expect(mockInvoke).not.toHaveBeenCalledWith("set_tor_enabled", expect.anything());
  });

  it("handleConnectivityCheck calls Tauri invoke", async () => {
    const user = userEvent.setup();

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Check Connectivity"));
    expect(mockInvoke).toHaveBeenCalledWith("check_connectivity");
  });

  it("handleResetStunDefaults calls set_stun_servers with defaults", async () => {
    const user = userEvent.setup();

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Reset STUN"));
    expect(mockInvoke).toHaveBeenCalledWith("set_stun_servers", {
      servers: ["stun.l.google.com:19302", "stun1.l.google.com:19302", "stun.cloudflare.com:3478", "stun.nextcloud.com:3478"],
    });
  });

  it("handleLanToggle calls set_discovery_config with lan_enabled: true", async () => {
    const user = userEvent.setup();
    // handleLanToggle uses hardcoded default {lan: false, dht: false} when null

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Toggle LAN"));
    expect(mockInvoke).toHaveBeenCalledWith("set_discovery_config", {
      config: { lan_enabled: true, dht_enabled: false },
    });
  });

  it("handleDhtToggle calls set_discovery_config with dht_enabled: true", async () => {
    const user = userEvent.setup();
    // handleDhtToggle uses hardcoded default {lan: false, dht: false} when null

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Toggle DHT"));
    expect(mockInvoke).toHaveBeenCalledWith("set_discovery_config", {
      config: { lan_enabled: false, dht_enabled: true },
    });
  });

  it("handleRefreshDiscovery calls refresh_discovery", async () => {
    const user = userEvent.setup();

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Refresh Discovery"));
    expect(mockInvoke).toHaveBeenCalledWith("refresh_discovery");
  });

  it("useSettings throws without SettingsProvider", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => render(<TestConsumer />)).toThrow();
    spy.mockRestore();
  });

  // ─── Security tests ───

  it("provides default security config", () => {
    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );
    expect(screen.getByTestId("screen-capture").textContent).toBe("false");
  });

  it("handleScreenCaptureToggle calls set_security_config", async () => {
    const user = userEvent.setup();
    // Relies on the command-aware default from `beforeEach`: the provider loads
    // a config with `screen_capture_protection: false` on mount, so the toggle
    // must flip it to `true`. (A blanket `mockResolvedValue` here also overrode
    // `get_security_config`, loading protection as already-on and making the
    // toggle send `false`.)
    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await waitFor(() =>
      expect(screen.getByTestId("screen-capture")).toHaveTextContent("false"),
    );

    await user.click(screen.getByText("Toggle Screen Capture"));

    expect(mockInvoke).toHaveBeenCalledWith("set_security_config", {
      config: { ...DEFAULT_SECURITY_CONFIG, screen_capture_protection: true },
    });
    await waitFor(() =>
      expect(screen.getByTestId("screen-capture")).toHaveTextContent("true"),
    );
  });

  it("handleLockVault calls lock_vault", async () => {
    const user = userEvent.setup();

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Lock Vault"));
    expect(mockInvoke).toHaveBeenCalledWith("lock_vault");
  });

  it("handleClearClipboard calls clear_clipboard", async () => {
    const user = userEvent.setup();

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Clear Clipboard"));
    expect(mockInvoke).toHaveBeenCalledWith("clear_clipboard");
  });

  // ─── Storage cap ─────────────────────────────────────────────────────────
  //
  // The cap decides how much of the user's history this app may destroy, so
  // "the dropdown changed" is not the property that matters — "the backend
  // holds the new number" is. A handler that updated local state and never
  // called `set_security_config` would leave the UI asserting a cap that is not
  // in force, which is the false-safety class this codebase keeps shipping.

  it("handleStorageCapChange persists the new cap in bytes", async () => {
    const user = userEvent.setup();
    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId("storage-cap")).toHaveTextContent("0"),
    );

    await user.click(screen.getByText("Set Cap 5GB"));

    expect(mockInvoke).toHaveBeenCalledWith("set_security_config", {
      config: { ...DEFAULT_SECURITY_CONFIG, storage_cap_bytes: 5 * 1024 ** 3 },
    });
    await waitFor(() =>
      expect(screen.getByTestId("storage-cap")).toHaveTextContent(String(5 * 1024 ** 3)),
    );
  });

  it("changing the cap re-reads usage so the row reflects the new ceiling", async () => {
    const user = userEvent.setup();
    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );
    mockInvoke.mockClear();

    await user.click(screen.getByText("Set Cap 5GB"));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("get_storage_usage"),
    );
  });

  it("reports a failed cap change instead of leaving the UI claiming success", async () => {
    // The optimistic-update-with-`catch {}` shape. A rejected `set_security_config`
    // must produce an error toast naming the failure; silently swallowing it
    // leaves the dropdown showing a cap the backend never adopted — so the
    // next eviction runs against a different number than the user chose.
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "set_security_config") return Promise.reject(new Error("disk read-only"));
      return defaultInvoke(cmd, args);
    });
    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId("storage-cap")).toHaveTextContent("0"),
    );

    await user.click(screen.getByText("Set Cap 5GB"));

    await waitFor(() =>
      expect(appState.addToast).toHaveBeenCalledWith(
        "Failed to update storage cap: disk read-only",
        "error",
      ),
    );
    // And the cap is unchanged, so the UI is not asserting a value the
    // backend rejected.
    expect(screen.getByTestId("storage-cap")).toHaveTextContent("0");
  });

  it("a failed usage read does not block the cap control", async () => {
    // `get_storage_usage` rejects; the row shows "Loading…" and the user must
    // still be able to raise the cap. A throw here would disable the one
    // control that stops the app destroying their history.
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "get_storage_usage") return Promise.reject(new Error("store not open"));
      return defaultInvoke(cmd, args);
    });
    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );
    await waitFor(() =>
      expect(screen.getByTestId("storage-usage")).toHaveTextContent("null"),
    );

    await user.click(screen.getByText("Set Cap 5GB"));

    expect(mockInvoke).toHaveBeenCalledWith("set_security_config", {
      config: { ...DEFAULT_SECURITY_CONFIG, storage_cap_bytes: 5 * 1024 ** 3 },
    });
  });
});
