import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Mock Tauri
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// Shared mock state that tests can mutate
const state = {
  identity: null as any,
  conversations: [] as any[],
  generatedInvite: "",
  inviteToConnect: "",
  inviteValid: false,
  isConnecting: false,
  namingMyName: "",
  namingTheirName: "",
  networkSettings: null as any,
  privateMode: false,
  toasts: [] as any[],
  handleGenerateInvite: vi.fn(),
  copyInvite: vi.fn(),
  handleConnect: vi.fn(),
  handleOpenChat: vi.fn(),
  handleDeleteConversation: vi.fn(),
  setInviteToConnect: vi.fn(),
  setNamingMyName: vi.fn(),
  setNamingTheirName: vi.fn(),
  openSettings: vi.fn(),
  removeToast: vi.fn(),
  addToast: vi.fn(),
  setView: vi.fn(),
  discoveryConfig: null as any,
  discoveredPeers: [] as any[],
};
};

vi.mock("../context/AppContext", () => ({
  useApp: () => ({
    identity: state.identity,
    toasts: state.toasts,
    removeToast: state.removeToast,
    addToast: state.addToast,
    setView: state.setView,
  }),
}));

vi.mock("../context/ChatContext", () => ({
  useChat: () => ({
    generatedInvite: state.generatedInvite,
    inviteToConnect: state.inviteToConnect,
    inviteValid: state.inviteValid,
    isConnecting: state.isConnecting,
    namingMyName: state.namingMyName,
    namingTheirName: state.namingTheirName,
    conversations: state.conversations,
    handleGenerateInvite: state.handleGenerateInvite,
    copyInvite: state.copyInvite,
    handleConnect: state.handleConnect,
    handleOpenChat: state.handleOpenChat,
    handleDeleteConversation: state.handleDeleteConversation,
    setInviteToConnect: state.setInviteToConnect,
    setNamingMyName: state.setNamingMyName,
    setNamingTheirName: state.setNamingTheirName,
  }),
}));

vi.mock("../context/SettingsContext", () => ({
  useSettings: () => ({
    networkSettings: state.networkSettings,
    privateMode: state.privateMode,
    openSettings: state.openSettings,
    discoveryConfig: state.discoveryConfig,
    discoveredPeers: state.discoveredPeers,
    handleConnectDiscoveredPeer: vi.fn(),
    handleRefreshDiscovery: vi.fn(),
    handleLanToggle: vi.fn(),
    handleDhtToggle: vi.fn(),
  }),
}));

import HubView from "../views/HubView";

describe("HubView", () => {
  beforeEach(() => {
    // Reset shared state between tests
    state.identity = null;
    state.conversations = [];
    state.generatedInvite = "";
    state.inviteToConnect = "";
    state.inviteValid = false;
    state.isConnecting = false;
    state.namingMyName = "";
    state.namingTheirName = "";
    state.networkSettings = null;
    state.privateMode = false;
    state.toasts = [];
    state.discoveryConfig = null;
    state.discoveredPeers = [];
    vi.clearAllMocks();
  });

  it("renders connect tab by default", () => {
    render(<HubView />);
    expect(screen.getByRole("tab", { name: /connect/i })).toHaveAttribute("aria-selected", "true");
  });

  it("renders the app title", () => {
    render(<HubView />);
    expect(screen.getByText("M2M")).toBeInTheDocument();
  });

  it("shows Offline badge by default", () => {
    render(<HubView />);
    expect(screen.getByText("Offline")).toBeInTheDocument();
  });

  it("renders settings button", () => {
    render(<HubView />);
    const settingsBtn = screen.getByLabelText("Settings");
    expect(settingsBtn).toBeInTheDocument();
  });

  it("calls openSettings when settings button clicked", async () => {
    const user = userEvent.setup();
    render(<HubView />);
    await user.click(screen.getByLabelText("Settings"));
    expect(state.openSettings).toHaveBeenCalledTimes(1);
  });

  it("renders generate invite button", () => {
    render(<HubView />);
    expect(screen.getByRole("button", { name: /generate invite/i })).toBeInTheDocument();
  });

  it("calls handleGenerateInvite when generate button clicked", async () => {
    const user = userEvent.setup();
    render(<HubView />);
    await user.click(screen.getByRole("button", { name: /generate invite/i }));
    expect(state.handleGenerateInvite).toHaveBeenCalledTimes(1);
  });

  it("shows invite link after generation", () => {
    state.generatedInvite = "m2m://test-invite-link";
    render(<HubView />);
    expect(screen.getByText("m2m://test-invite-link")).toBeInTheDocument();
  });

  it("shows copy invite button when invite generated", () => {
    state.generatedInvite = "m2m://test-invite";
    render(<HubView />);
    expect(screen.getByLabelText("Copy invite")).toBeInTheDocument();
  });

  it("shows invite input for joining", () => {
    render(<HubView />);
    expect(screen.getByPlaceholderText("m2m://...")).toBeInTheDocument();
  });

  it("disables connect button when no invite text", () => {
    render(<HubView />);
    const connectBtn = screen.getByRole("button", { name: /connect/i });
    expect(connectBtn).toBeDisabled();
  });

  it("disables connect button while connecting", () => {
    state.isConnecting = true;
    render(<HubView />);
    // When loading, the button shows a spinner, not "Connect" text
    // Use getAllByRole to find the disabled button
    const buttons = screen.getAllByRole("button");
    const connectBtn = buttons.find(b => b.id === "connect-btn");
    expect(connectBtn).toBeDefined();
    expect(connectBtn).toBeDisabled();
  });

  it("calls handleConnect when connect button clicked with invite", async () => {
    state.inviteToConnect = "m2m://valid-invite";
    const user = userEvent.setup();
    render(<HubView />);
    const connectBtn = screen.getByRole("button", { name: /connect/i });
    expect(connectBtn).not.toBeDisabled();
    await user.click(connectBtn);
    expect(state.handleConnect).toHaveBeenCalledTimes(1);
  });

  it("shows naming panel when invite is valid", () => {
    state.inviteValid = true;
    render(<HubView />);
    expect(screen.getByText("Valid Invite Found")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("How they will see you")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("How you want to see them")).toBeInTheDocument();
  });

  it("switches to chats tab on click", async () => {
    const user = userEvent.setup();
    render(<HubView />);
    await user.click(screen.getByRole("tab", { name: /chats/i }));
    expect(screen.getByRole("tab", { name: /chats/i })).toHaveAttribute("aria-selected", "true");
  });

  it("shows chats tab with conversation list", async () => {
    state.conversations = [
      { id: "conv-1", peer_display_name: "Alice", peer_key_hex: "abc123", last_message_preview: "Hello!", last_message_at: Date.now() / 1000 },
    ];
    render(<HubView />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("tab", { name: /chats/i }));
    expect(screen.getByText("Alice")).toBeInTheDocument();
  });

  it("shows empty state when no conversations", async () => {
    render(<HubView />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("tab", { name: /chats/i }));
    expect(screen.getByText(/no conversations yet/i)).toBeInTheDocument();
  });

  it("shows fingerprint display section", () => {
    state.identity = {
      fingerprint: "AB12-CD34-EF56-7890",
      public_key_hex: "abc123",
      has_identity: true,
    };
    render(<HubView />);
    expect(screen.getByText("AB12-CD34-EF56-7890")).toBeInTheDocument();
  });

  it("shows Tor warning when Tor enabled without private mode", () => {
    state.generatedInvite = "m2m://test";
    state.networkSettings = { tor_enabled: true, public_ip: "1.2.3.4" };
    state.privateMode = false;
    render(<HubView />);
    expect(screen.getByText(/Tor Inbound Warning/i)).toBeInTheDocument();
  });

  it("hides Tor warning when private mode is on", () => {
    state.generatedInvite = "m2m://test";
    state.networkSettings = { tor_enabled: true, public_ip: "1.2.3.4" };
    state.privateMode = true;
    render(<HubView />);
    expect(screen.queryByText(/Tor Inbound Warning/i)).not.toBeInTheDocument();
  });
});
