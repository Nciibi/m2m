import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, screen, waitFor } from "@testing-library/react";
import { render } from "./setup";
import userEvent from "@testing-library/user-event";
import { useApp, AppProvider } from "../context/AppContext";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const { eventHandlers } = vi.hoisted(() => ({
  eventHandlers: new Map<string, () => void>(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: () => void) => {
    eventHandlers.set(name, handler);
    return Promise.resolve(() => eventHandlers.delete(name));
  }),
}));

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

  it("starts at setup and then routes to the hub once the vault is read", async () => {
    const invoke = (await import("@tauri-apps/api/core")).invoke as ReturnType<typeof vi.fn>;
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_vault_status") return { initialized: true, unlocked: true };
      if (cmd === "get_identity") return { fingerprint: "ABCD", public_key_hex: "ff", has_identity: true };
      return null;
    });

    render(
      <AppProvider>
        <TestConsumer />
      </AppProvider>
    );
    // Before the async status read resolves the view is still "setup".
    expect(screen.getByTestId("view")).toHaveTextContent("setup");
    await waitFor(() => expect(screen.getByTestId("view")).toHaveTextContent("hub"));
    expect(screen.getByTestId("identity")).toHaveTextContent("ABCD");
    expect(screen.getByTestId("vault-initialized")).toHaveTextContent("true");
  });

  it("routes to vault on lock and rejects late protected navigation", async () => {
    const invoke = (await import("@tauri-apps/api/core")).invoke as ReturnType<typeof vi.fn>;
    invoke
      .mockResolvedValueOnce({ fingerprint: "ABCD", public_key_hex: "ff", has_identity: true })
      .mockResolvedValueOnce({ initialized: true, unlocked: true });
    const user = userEvent.setup();

    render(<AppProvider><TestConsumer /></AppProvider>);
    await waitFor(() => expect(screen.getByTestId("view")).toHaveTextContent("hub"));
    await user.click(screen.getByText("Set Chat"));
    await user.click(screen.getByText("Add Toast"));

    act(() => { eventHandlers.get("m2m://vault-locked")?.(); });

    expect(screen.getByTestId("view")).toHaveTextContent("vault");
    expect(screen.getByTestId("identity")).toHaveTextContent("none");
    expect(screen.getByTestId("toast-count")).toHaveTextContent("0");
    await user.click(screen.getByText("Set Chat"));
    expect(screen.getByTestId("view")).toHaveTextContent("vault");
  });

  it("allows setting view when the vault is unlocked", async () => {
    const user = userEvent.setup();
    const invoke = (await import("@tauri-apps/api/core")).invoke as ReturnType<typeof vi.fn>;
    // `get_vault_status` must report the vault as UNLOCKED. The previous mock
    // omitted `unlocked` entirely, so the navigation guard (correctly) rejected
    // "chat" and this assertion was passing for the wrong reason — or failing,
    // depending on timing. Navigation away from the lock screen is gated on the
    // vault being unlocked, so the test has to say so.
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_vault_status") return { initialized: true, unlocked: true };
      if (cmd === "get_identity") return { fingerprint: "ABCD", public_key_hex: "ff", has_identity: true };
      return null;
    });

    render(
      <AppProvider>
        <TestConsumer />
      </AppProvider>
    );
    await waitFor(() => expect(screen.getByTestId("view")).toHaveTextContent("hub"));
    await user.click(screen.getByText("Set Chat"));
    expect(screen.getByTestId("view").textContent).toBe("chat");
  });

  it("blocks navigation away from the lock screen while locked", async () => {
    // The other half of the guard: a locked vault must NOT be navigable.
    const user = userEvent.setup();
    const invoke = (await import("@tauri-apps/api/core")).invoke as ReturnType<typeof vi.fn>;
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_vault_status") return { initialized: true, unlocked: false };
      return null;
    });

    render(
      <AppProvider>
        <TestConsumer />
      </AppProvider>
    );
    await waitFor(() => expect(screen.getByTestId("view")).toHaveTextContent("vault"));
    await user.click(screen.getByText("Set Chat"));
    expect(screen.getByTestId("view").textContent).toBe("vault");
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
    // This test previously had an empty body — it rendered and asserted
    // nothing, so it could never fail. It now verifies the actual contract:
    // an unlocked, initialized vault lands the user on the hub with their
    // identity loaded.
    const invoke = (await import("@tauri-apps/api/core")).invoke as ReturnType<typeof vi.fn>;
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_vault_status") return { initialized: true, unlocked: true };
      if (cmd === "get_identity") return { fingerprint: "ABCD", public_key_hex: "ff", has_identity: true };
      return null;
    });

    render(
      <AppProvider>
        <TestConsumer />
      </AppProvider>
    );
    await waitFor(() => expect(screen.getByTestId("view")).toHaveTextContent("hub"));
    expect(screen.getByTestId("vault-initialized")).toHaveTextContent("true");
    expect(screen.getByTestId("identity")).toHaveTextContent("ABCD");
  });
});
