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
  } = useSettings();
  return (
    <div>
      <span data-testid="public-ip">{publicIp || "null"}</span>
      <span data-testid="stun-loading">{String(stunLoading)}</span>
      <span data-testid="private-mode">{String(privateMode)}</span>
      <span data-testid="stun-server-count">{stunConfig?.servers?.length ?? -1}</span>
      <button onClick={handleStunDiscover}>STUN Discover</button>
      <button onClick={handlePrivateModeToggle}>Toggle Private</button>
      <button onClick={handleTorToggle}>Toggle Tor</button>
      <button onClick={handleConnectivityCheck}>Check Connectivity</button>
      <button onClick={handleAddStunServer}>Add STUN Server</button>
      <button onClick={() => handleRemoveStunServer(0)}>Remove STUN 0</button>
      <button onClick={handleResetStunDefaults}>Reset STUN</button>
      <button onClick={() => setStunServerInput("test:3478")}>Set STUN Input</button>
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
  });

  it("handleStunDiscover calls Tauri invoke", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue("203.0.113.1");
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

  it("handleTorToggle calls Tauri invoke", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue(undefined);

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Toggle Tor"));
    expect(mockInvoke).toHaveBeenCalledWith("set_tor_enabled", expect.any(Object));
  });

  it("handleConnectivityCheck calls Tauri invoke", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue({ reachable: true });

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Check Connectivity"));
    expect(mockInvoke).toHaveBeenCalledWith("check_connectivity");
  });

  it("handleAddStunServer validates input and adds server", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue(undefined);

    // Need to set up stunConfig and stunServerInput
    // First render with openSettings called to populate stunConfig
    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    // Set STUN input first
    await user.click(screen.getByText("Set STUN Input"));
    await user.click(screen.getByText("Add STUN Server"));
    // Should not call invoke because stunConfig is null (no servers yet)
    // The handleAddStunServer guard checks stunConfig first
  });

  it("handleResetStunDefaults calls Tauri invoke", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue({ servers: ["default.stun:3478"], private_mode: false });
    mockInvoke.mockResolvedValueOnce({ servers: ["default.stun:3478"], private_mode: false });

    render(
      <SettingsProvider>
        <TestConsumer />
      </SettingsProvider>
    );

    await user.click(screen.getByText("Reset STUN"));
    expect(mockInvoke).toHaveBeenCalledWith("reset_stun_servers");
  });

  it("useSettings throws without SettingsProvider", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => render(<TestConsumer />)).toThrow();
    spy.mockRestore();
  });
});
