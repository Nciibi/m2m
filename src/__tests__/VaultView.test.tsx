import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Mock the entire @tauri-apps/api/core module
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Mock the M2M context with test values
const mockM2MContext = {
  vaultInitialized: false,
  handleUnlockVault: vi.fn(),
  toasts: [],
  removeToast: vi.fn(),
  addToast: vi.fn(),
  view: "vault" as const,
  setView: vi.fn(),
  identity: null,
  connection: null,
  isConnecting: false,
  messages: [],
  fileRequests: [],
  networkSettings: null,
  publicIp: null,
  stunLoading: false,
  networkDiagnostics: null,
  stunConfig: null,
  stunServerInput: "",
  privateMode: false,
  connectivityResult: null,
  conversations: [],
  activeConversationId: null,
  inviteToConnect: "",
  inviteValid: false,
  namingMyName: "",
  namingTheirName: "",
  generatedInvite: "",
  retentionPolicy: "none",
  retentionDuration: "86400",
  setStunServerInput: vi.fn(),
  setInviteToConnect: vi.fn(),
  setNamingMyName: vi.fn(),
  setNamingTheirName: vi.fn(),
  setRetentionPolicy: vi.fn(),
  setRetentionDuration: vi.fn(),
  handleSendMessage: vi.fn(),
  handleVerify: vi.fn(),
  handleDisconnect: vi.fn(),
  handleSendFile: vi.fn(),
  handleExportConversation: vi.fn(),
  handleSetRetention: vi.fn(),
  openSettings: vi.fn(),
  handleGenerateInvite: vi.fn(),
  copyInvite: vi.fn(),
  handleConnect: vi.fn(),
  handleOpenChat: vi.fn(),
  handleStunDiscover: vi.fn(),
  handleAddStunServer: vi.fn(),
  handleRemoveStunServer: vi.fn(),
  handleResetStunDefaults: vi.fn(),
  handlePrivateModeToggle: vi.fn(),
  handleConnectivityCheck: vi.fn(),
  handleTorToggle: vi.fn(),
  handleDeleteConversation: vi.fn(),
};

vi.mock("../context/M2MContext", () => ({
  useM2M: () => mockM2MContext,
  M2MProvider: ({ children }: { children: React.ReactNode }) => children,
}));

import VaultView from "../views/VaultView";

describe("VaultView", () => {
  it("renders the set up title for first-time users", () => {
    render(<VaultView />);
    expect(screen.getByText("Set Up Your Vault")).toBeInTheDocument();
  });

  it("renders a passphrase input field", () => {
    render(<VaultView />);
    const input = screen.getByPlaceholderText("Passphrase");
    expect(input).toBeInTheDocument();
    expect(input).toHaveAttribute("type", "password");
  });

  it("renders a confirm passphrase input for first-time users", () => {
    render(<VaultView />);
    expect(screen.getByPlaceholderText("Confirm passphrase")).toBeInTheDocument();
  });

  it("renders unlock/create button", () => {
    render(<VaultView />);
    expect(screen.getByText("Create Vault")).toBeInTheDocument();
  });

  it("shows unlock title for returning users", () => {
    mockM2MContext.vaultInitialized = true;
    render(<VaultView />);
    expect(screen.getByText("Unlock Your Vault")).toBeInTheDocument();
    mockM2MContext.vaultInitialized = false;
  });

  it("does not show confirm input for returning users", () => {
    mockM2MContext.vaultInitialized = true;
    render(<VaultView />);
    expect(screen.queryByPlaceholderText("Confirm passphrase")).not.toBeInTheDocument();
    mockM2MContext.vaultInitialized = false;
  });

  it("shows passphrase tips when toggled", async () => {
    const user = userEvent.setup();
    render(<VaultView />);
    const toggle = screen.getByText("What makes a strong passphrase?");
    await user.click(toggle);
    expect(screen.getByText(/5\+ random words/)).toBeInTheDocument();
  });

  it("shows strength meter as user types", async () => {
    const user = userEvent.setup();
    render(<VaultView />);
    const input = screen.getByPlaceholderText("Passphrase");
    await user.type(input, "correct-horse-battery-staple");
    // Should show the strength info
    expect(screen.getByText(/chars/)).toBeInTheDocument();
  });
});
