import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Mock the entire @tauri-apps/api/core module
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// VaultView uses useApp() + useVault() now
const appState = {
  vaultInitialized: false,
  toasts: [],
  removeToast: vi.fn(),
  addToast: vi.fn(),
  view: "vault" as const,
  setView: vi.fn(),
  identity: null,
};

const vaultState = {
  handleUnlockVault: vi.fn(),
};

vi.mock("../context/AppContext", () => ({
  useApp: () => appState,
}));

vi.mock("../context/VaultContext", () => ({
  useVault: () => vaultState,
}));

import VaultView from "../views/VaultView";

describe("VaultView", () => {
  beforeEach(() => {
    appState.vaultInitialized = false;
    appState.toasts = [];
    vi.clearAllMocks();
  });

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
    appState.vaultInitialized = true;
    render(<VaultView />);
    expect(screen.getByText("Unlock Your Vault")).toBeInTheDocument();
  });

  it("does not show confirm input for returning users", () => {
    appState.vaultInitialized = true;
    render(<VaultView />);
    expect(screen.queryByPlaceholderText("Confirm passphrase")).not.toBeInTheDocument();
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
    // Shows "28 chars" in the strength section
    expect(screen.getByText("28")).toBeInTheDocument();
  });
});
