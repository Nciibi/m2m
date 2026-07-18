import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// State for SettingsContext mock
const settingsState = {
  networkSettings: null as any,
  publicIp: null as string | null,
  stunLoading: false,
  networkDiagnostics: null as any,
  stunConfig: null as any,
  stunServerInput: "",
  privateMode: false,
  connectivityResult: null as any,
  openSettings: vi.fn(),
  handleStunDiscover: vi.fn(),
  handleAddStunServer: vi.fn(),
  handleRemoveStunServer: vi.fn(),
  handleResetStunDefaults: vi.fn(),
  handlePrivateModeToggle: vi.fn(),
  handleConnectivityCheck: vi.fn(),
  handleTorToggle: vi.fn(),
  setStunServerInput: vi.fn(),
  discoveryConfig: null as any,
  discoveredPeers: [] as any[],
  handleLanToggle: vi.fn(),
  handleDhtToggle: vi.fn(),
  handleConnectDiscoveredPeer: vi.fn(),
  handleRefreshDiscovery: vi.fn(),
};

// State for AppContext mock
const appState = {
  identity: null as any,
  toasts: [] as any[],
  removeToast: vi.fn(),
  addToast: vi.fn(),
  setView: vi.fn(),
};

vi.mock("../context/AppContext", () => ({
  useApp: () => ({
    identity: appState.identity,
    toasts: appState.toasts,
    removeToast: appState.removeToast,
    addToast: appState.addToast,
    setView: appState.setView,
  }),
}));

vi.mock("../context/SettingsContext", () => ({
  useSettings: () => ({
    networkSettings: settingsState.networkSettings,
    publicIp: settingsState.publicIp,
    stunLoading: settingsState.stunLoading,
    networkDiagnostics: settingsState.networkDiagnostics,
    stunConfig: settingsState.stunConfig,
    stunServerInput: settingsState.stunServerInput,
    privateMode: settingsState.privateMode,
    connectivityResult: settingsState.connectivityResult,
    openSettings: settingsState.openSettings,
    handleStunDiscover: settingsState.handleStunDiscover,
    handleAddStunServer: settingsState.handleAddStunServer,
    handleRemoveStunServer: settingsState.handleRemoveStunServer,
    handleResetStunDefaults: settingsState.handleResetStunDefaults,
    handlePrivateModeToggle: settingsState.handlePrivateModeToggle,
    handleConnectivityCheck: settingsState.handleConnectivityCheck,
    handleTorToggle: settingsState.handleTorToggle,
    setStunServerInput: settingsState.setStunServerInput,
    discoveryConfig: settingsState.discoveryConfig,
    discoveredPeers: settingsState.discoveredPeers,
    handleLanToggle: settingsState.handleLanToggle,
    handleDhtToggle: settingsState.handleDhtToggle,
    handleConnectDiscoveredPeer: settingsState.handleConnectDiscoveredPeer,
    handleRefreshDiscovery: settingsState.handleRefreshDiscovery,
  }),
}));

vi.mock("../context/ThemeContext", () => ({
  useTheme: () => ({
    theme: "system",
    setTheme: vi.fn(),
    resolvedTheme: "dark",
    accentColor: "#6366f1",
    setAccentColor: vi.fn(),
  }),
}));

import SettingsView from "../views/SettingsView";

describe("SettingsView", () => {
  beforeEach(() => {
    settingsState.networkSettings = null;
    settingsState.publicIp = null;
    settingsState.stunLoading = false;
    settingsState.networkDiagnostics = null;
    settingsState.stunConfig = null;
    settingsState.stunServerInput = "";
    settingsState.privateMode = false;
    settingsState.connectivityResult = null;
    appState.identity = null;
    appState.toasts = [];
    vi.clearAllMocks();
  });

  it("renders the settings title", () => {
    render(<SettingsView />);
    // The sidebar also has a "Settings" nav item, so scope to the header heading.
    expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
  });

  it("shows back to hub button", () => {
    render(<SettingsView />);
    expect(screen.getByRole("button", { name: /hub/i })).toBeInTheDocument();
  });

  it("navigates to hub on back button click", async () => {
    const user = userEvent.setup();
    render(<SettingsView />);
    await user.click(screen.getByRole("button", { name: /hub/i }));
    expect(appState.setView).toHaveBeenCalledWith("hub");
  });

  it("renders identity section", () => {
    appState.identity = {
      fingerprint: "AB12-CD34-EF56-7890",
      public_key_hex: "abc123def456",
      has_identity: true,
    };
    render(<SettingsView />);
    expect(screen.getByText("AB12-CD34-EF56-7890")).toBeInTheDocument();
    expect(screen.getByText("abc123def456")).toBeInTheDocument();
  });

  it("shows placeholder when no identity", () => {
    render(<SettingsView />);
    const placeholders = screen.getAllByText("—");
    expect(placeholders.length).toBeGreaterThanOrEqual(2);
  });

  it("renders network section with public IP discovery", () => {
    render(<SettingsView />);
    expect(screen.getByText("Public IP")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /discover via stun/i })).toBeInTheDocument();
  });

  it("shows public IP when available", () => {
    settingsState.publicIp = "203.0.113.42";
    render(<SettingsView />);
    expect(screen.getByText("203.0.113.42")).toBeInTheDocument();
  });

  it("shows NAT type badge when diagnostics available", () => {
    settingsState.networkDiagnostics = { nat_type: "RestrictedCone", stun_servers: [{ reachable: true }] };
    render(<SettingsView />);
    expect(screen.getByText("RestrictedCone")).toBeInTheDocument();
    expect(screen.getByText("1/1 reachable")).toBeInTheDocument();
  });

  it("shows warning NAT type badge for restricted NATs", () => {
    settingsState.networkDiagnostics = { nat_type: "Symmetric", stun_servers: [] };
    render(<SettingsView />);
    expect(screen.getByText("Symmetric")).toBeInTheDocument();
  });

  it("renders private mode toggle", () => {
    render(<SettingsView />);
    expect(screen.getByLabelText("Toggle private mode")).toBeInTheDocument();
  });

  it("calls handlePrivateModeToggle when toggled", async () => {
    const user = userEvent.setup();
    render(<SettingsView />);
    await user.click(screen.getByLabelText("Toggle private mode"));
    expect(settingsState.handlePrivateModeToggle).toHaveBeenCalledTimes(1);
  });

  it("renders Tor toggle", () => {
    render(<SettingsView />);
    expect(screen.getByLabelText("Toggle Tor")).toBeInTheDocument();
  });

  it("renders connectivity check button", () => {
    render(<SettingsView />);
    expect(screen.getByRole("button", { name: /check/i })).toBeInTheDocument();
  });

  it("calls handleConnectivityCheck when check clicked", async () => {
    const user = userEvent.setup();
    render(<SettingsView />);
    await user.click(screen.getByRole("button", { name: /check/i }));
    expect(settingsState.handleConnectivityCheck).toHaveBeenCalledTimes(1);
  });

  it("shows connectivity result when available", () => {
    settingsState.connectivityResult = { reachable: true, latency_ms: 42 };
    render(<SettingsView />);
    expect(screen.getByText(/reachable/)).toBeInTheDocument();
  });

  it("renders STUN servers section", () => {
    settingsState.stunConfig = { servers: ["stun.l.google.com:19302", "stun1.l.google.com:19302"], private_mode: false };
    render(<SettingsView />);
    expect(screen.getByText("stun.l.google.com:19302")).toBeInTheDocument();
    expect(screen.getByText("stun1.l.google.com:19302")).toBeInTheDocument();
  });

  it("renders add STUN server input", () => {
    render(<SettingsView />);
    expect(screen.getByPlaceholderText("host:port")).toBeInTheDocument();
  });

  it("adds STUN server", async () => {
    const user = userEvent.setup();
    settingsState.stunConfig = { servers: [], private_mode: false };
    settingsState.stunServerInput = "stun.example.com:3478";
    render(<SettingsView />);
    const addBtn = screen.getByRole("button", { name: /add/i });
    await user.click(addBtn);
    expect(settingsState.handleAddStunServer).toHaveBeenCalledTimes(1);
  });

  it("removes STUN server", async () => {
    const user = userEvent.setup();
    settingsState.stunConfig = { servers: ["stun.l.google.com:19302"], private_mode: false };
    render(<SettingsView />);
    const removeBtn = screen.getByLabelText("Remove STUN server");
    await user.click(removeBtn);
    expect(settingsState.handleRemoveStunServer).toHaveBeenCalledWith(0);
  });

  it("resets STUN defaults", async () => {
    const user = userEvent.setup();
    settingsState.stunConfig = { servers: ["stun.l.google.com:19302"], private_mode: false };
    render(<SettingsView />);
    await user.click(screen.getByRole("button", { name: /reset stun servers to defaults/i }));
    expect(settingsState.handleResetStunDefaults).toHaveBeenCalledTimes(1);
  });

  it("shows version info", () => {
    render(<SettingsView />);
    expect(screen.getByText("Version")).toBeInTheDocument();
  });

  it("renders fingerprint copy button", () => {
    appState.identity = { fingerprint: "ABCD-1234", public_key_hex: "ff", has_identity: true };
    render(<SettingsView />);
    expect(screen.getByLabelText("Copy fingerprint")).toBeInTheDocument();
  });
});
