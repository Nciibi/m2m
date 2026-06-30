import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: any[]) => mockInvoke(...args) }));

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
    networkSettings, publicIp, stunLoading, networkDiagnostics,
    stunConfig, stunServerInput, privateMode, connectivityResult,
    handleStunDiscover, handleAddStunServer, handleRemoveStunServer,
    handleResetStunDefaults, handlePrivateModeToggle, handleConnectivityCheck,
    handleTorToggle, setStunServerInput,
    discoveryConfig, discoveredPeers,
    handleLanToggle, handleDhtToggle, handleRefreshDiscovery,
    securityConfig,
    handleScreenCaptureToggle, handleLockVault, handleClearClipboard,
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
    </div>
  );
}

describe("SettingsContext", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    appState.addToast.mockClear();
    appState.setView.mockClear();
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
    mockInvoke.mockResolvedValueOnce("203.0.113.1");
    mockInvoke.mockResolvedValueOnce({ nat_type: "RestrictedCone", stun_servers: [] });

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
    mockInvoke.mockResolvedValue(undefined);

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
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("handleConnectivityCheck calls Tauri invoke", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue({ reachable: true });
    mockInvoke.mockResolvedValueOnce({ reachable: true });
    mockInvoke.mockResolvedValueOnce({ nat_type: "FullCone", stun_servers: [] });

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
    mockInvoke.mockResolvedValue(undefined);

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
    mockInvoke.mockResolvedValueOnce({ lan_enabled: true, dht_enabled: false });
    mockInvoke.mockResolvedValueOnce([]);

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
    mockInvoke.mockResolvedValueOnce({ lan_enabled: false, dht_enabled: true });
    mockInvoke.mockResolvedValueOnce([]);

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
    mockInvoke.mockResolvedValueOnce([]);

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
    mockInvoke.mockResolvedValue({ screen_capture_protection: true, clipboard_clear_secs: 0, idle_lock_secs: 0 });

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Toggle Screen Capture"));
    expect(mockInvoke).toHaveBeenCalledWith("set_security_config", {
      config: { screen_capture_protection: true, clipboard_clear_secs: 0, idle_lock_secs: 0 },
    });
  });

  it("handleLockVault calls lock_vault", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue(undefined);

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
    mockInvoke.mockResolvedValue(undefined);

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Clear Clipboard"));
    expect(mockInvoke).toHaveBeenCalledWith("clear_clipboard");
  });
});
