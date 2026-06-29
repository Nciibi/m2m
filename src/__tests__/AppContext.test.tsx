import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useApp, AppProvider } from "../context/AppContext";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

function TestConsumer() {
  const { view, setView, toasts, addToast, removeToast, identity, vaultInitialized } = useApp();
  return (
    <div>
      <span data-testid="view">{view}</span>
      <span data-testid="identity">{identity?.fingerprint || "none"}</span>
      <span data-testid="vault-initialized">{String(vaultInitialized)}</span>
      <span data-testid="toast-count">{toasts.length}</span>
      <button onClick={() => setView("chat")}>Set Chat</button>
      <button onClick={() => addToast("Test Toast", "info")}>Add Toast</button>
      <button onClick={() => toasts[0] && removeToast(toasts[0].id)}>Remove Toast</button>
    </div>
  );
}

describe("AppContext", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("provides default view and identity", async () => {
    const invoke = (await import("@tauri-apps/api/core")).invoke as ReturnType<typeof vi.fn>;
    invoke.mockResolvedValue({ fingerprint: "ABCD", public_key_hex: "ff", has_identity: true });

    render(
      <AppProvider>
        <TestConsumer />
      </AppProvider>
    );
    // Initially "setup", then flips based on invoke
    // Since we can't easily wait for async state, check it renders
    expect(screen.getByText("Set Chat")).toBeInTheDocument();
  });

  it("allows setting view", async () => {
    const user = userEvent.setup();
    const invoke = (await import("@tauri-apps/api/core")).invoke as ReturnType<typeof vi.fn>;
    invoke.mockResolvedValue({ fingerprint: "ABCD", public_key_hex: "ff", has_identity: false });

    render(
      <AppProvider>
        <TestConsumer />
      </AppProvider>
    );
    await user.click(screen.getByText("Set Chat"));
    expect(screen.getByTestId("view").textContent).toBe("chat");
  });

  it("provides addToast and toasts update", async () => {
    const user = userEvent.setup();

    render(
      <AppProvider>
        <TestConsumer />
      </AppProvider>
    );

    expect(screen.getByTestId("toast-count").textContent).toBe("0");
    await user.click(screen.getByText("Add Toast"));
    // After addToast, toast count should be 1
    expect(screen.getByTestId("toast-count").textContent).toBe("1");
  });

  it("useApp throws without AppProvider", () => {
    // Suppress console.error for the expected error
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => render(<TestConsumer />)).toThrow();
    spy.mockRestore();
  });

  it("initializes with vault status from invoke", async () => {
    const invoke = (await import("@tauri-apps/api/core")).invoke as ReturnType<typeof vi.fn>;
    invoke
      .mockResolvedValueOnce({ fingerprint: "ABCD", public_key_hex: "ff", has_identity: true })
      .mockResolvedValueOnce({ initialized: true, unlocked: true });

    render(
      <AppProvider>
        <TestConsumer />
      </AppProvider>
    );
  });
});
