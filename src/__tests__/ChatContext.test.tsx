import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: any[]) => mockInvoke(...args) }));
// Capture the registered event handlers so a test can simulate a peer
// connecting — the trust-anchor behaviour under test only applies once
// `connection.peer_key_hex` is set.
const { eventHandlers } = vi.hoisted(() => ({
  eventHandlers: new Map<string, (e: any) => void>(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: (e: any) => void) => {
    eventHandlers.set(name, handler);
    return Promise.resolve(() => eventHandlers.delete(name));
  }),
}));

// Mock AppContext used by ChatProvider
const appState = {
  addToast: vi.fn(),
  setView: vi.fn(),
};
vi.mock("../context/AppContext", () => ({
  useApp: () => appState,
}));

import { ChatProvider, useChat } from "../context/ChatContext";

function TestConsumer() {
  const {
    connection, isConnecting, messages, conversations, fileRequests,
    handleGenerateInvite,
    handleOpenChat, handleDeleteConversation, setInviteToConnect,
    copyInvite, handleVerify, handleSendFile, handleExportConversation,
    handleSendReaction, handleRemoveReaction, handleMarkConversationRead,
  } = useChat();
  const [verifyError, setVerifyError] = useState<string | null>(null);
  return (
    <div>
      <span data-testid="connection-state">{connection?.state || "null"}</span>
      <span data-testid="peer-verified">{String(connection?.peer_verified ?? "unset")}</span>
      <span data-testid="verify-error">{verifyError ?? "none"}</span>
      <span data-testid="is-connecting">{String(isConnecting)}</span>
      <span data-testid="messages-count">{messages.length}</span>
      <span data-testid="conversations-count">{conversations.length}</span>
      <span data-testid="file-requests-count">{fileRequests.length}</span>
      <button onClick={handleGenerateInvite}>Generate Invite</button>
      <button onClick={() => setInviteToConnect("m2m://test")}>Set Invite</button>
      <button onClick={copyInvite}>Copy Invite</button>
      <button onClick={async () => {
        // Mirror what ChatView does: only report success once the backend has
        // actually persisted the verification.
        try {
          await handleVerify();
          setVerifyError(null);
        } catch (e) {
          setVerifyError(String(e));
        }
      }}>Verify</button>
      <button onClick={handleSendFile}>Send File</button>
      <button onClick={handleExportConversation}>Export</button>
      <button onClick={handleDeleteConversation}>Delete Conv</button>
      <button onClick={() => handleOpenChat({ id: "c1", peer_key_hex: "abc", display_name: null, peer_display_name: null, last_message_at: null, last_message_preview: null, message_count: 0, is_online: false, auto_delete_at: null, retention_policy: "none", created_at: 0 })}>Open Chat</button>
      <button onClick={() => handleSendReaction("msg-1", "👍")}>Send Reaction</button>
      <button onClick={() => handleRemoveReaction("msg-1", "👍")}>Remove Reaction</button>
      <button onClick={handleMarkConversationRead}>Mark Read</button>
    </div>
  );
}

describe("ChatContext", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    appState.addToast.mockClear();
    appState.setView.mockClear();
  });

  it("provides default connection state", () => {
    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );
    expect(screen.getByTestId("connection-state").textContent).toBe("null");
    expect(screen.getByTestId("is-connecting").textContent).toBe("false");
    expect(screen.getByTestId("messages-count").textContent).toBe("0");
    expect(screen.getByTestId("conversations-count").textContent).toBe("0");
    expect(screen.getByTestId("file-requests-count").textContent).toBe("0");
  });

  it("handleGenerateInvite calls Tauri invoke", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue("m2m://generated-invite");

    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    await user.click(screen.getByText("Generate Invite"));
    expect(mockInvoke).toHaveBeenCalled();
  });

  it("handleOpenChat loads messages from invoke", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue([]); // messages list

    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    await user.click(screen.getByText("Open Chat"));
    expect(mockInvoke).toHaveBeenCalledWith("load_messages", expect.any(Object));
  });

  it("useChat throws without ChatProvider", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => render(<TestConsumer />)).toThrow();
    spy.mockRestore();
  });

  it("sets inviteToConnect via setter", async () => {
    const user = userEvent.setup();
    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    await user.click(screen.getByText("Set Invite"));
    expect(screen.getByText("Set Invite")).toBeInTheDocument();
  });

  // ─── Reaction tests ───

  it("handleSendReaction calls Tauri invoke with reaction args", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue([]); // default for load_messages
    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    // Need connection first — open a chat
    await user.click(screen.getByText("Open Chat"));
    // Clear call history so we only check the reaction call
    mockInvoke.mockClear();
    mockInvoke.mockResolvedValue(undefined);

    await user.click(screen.getByText("Send Reaction"));
    expect(mockInvoke).toHaveBeenCalledWith("send_reaction", {
      peerKeyHex: "abc",
      messageId: "msg-1",
      reaction: "👍",
    });
  });

  it("handleRemoveReaction calls Tauri invoke with remove_reaction", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue([]); // default for load_messages
    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    // Need connection first — open a chat
    await user.click(screen.getByText("Open Chat"));
    mockInvoke.mockClear();
    mockInvoke.mockResolvedValue(undefined);

    await user.click(screen.getByText("Remove Reaction"));
    expect(mockInvoke).toHaveBeenCalledWith("remove_reaction", {
      peerKeyHex: "abc",
      messageId: "msg-1",
      reaction: "👍",
    });
  });

  it("handleMarkConversationRead calls mark_messages_read", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue([]); // default for load_messages
    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    // Need activeConversationId first — open a chat
    await user.click(screen.getByText("Open Chat"));
    mockInvoke.mockClear();
    mockInvoke.mockResolvedValue(0);

    await user.click(screen.getByText("Mark Read"));
    expect(mockInvoke).toHaveBeenCalledWith("mark_messages_read", {
      conversationId: "abc",
    });
  });

  it("reaction handlers are no-ops when no connection", async () => {
    const user = userEvent.setup();
    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    // Ignore mount-effect invokes (e.g. get_muted_conversations); assert only
    // the reaction handler makes no call when there is no active connection.
    mockInvoke.mockClear();
    await user.click(screen.getByText("Send Reaction"));
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  /** Simulate a peer completing a handshake. */
  async function establishConnection() {
    await waitFor(() => expect(eventHandlers.get("m2m://connection")).toBeDefined());
    act(() => {
      eventHandlers.get("m2m://connection")?.({
        payload: {
          state: "established",
          peer_key_hex: "aabbcc",
          peer_fingerprint: "AA:BB:CC",
          peer_verified: false,
        },
      });
    });
    await waitFor(() =>
      expect(screen.getByTestId("connection-state")).toHaveTextContent("established"),
    );
  }

  /**
   * Trust-anchor regression.
   *
   * `handleVerify` used to swallow its own error, and ChatView did
   * `await handleVerify(); addToast("Peer verified", "success")` — so a
   * *failed* verification still showed a green success confirmation, and the
   * badge silently stayed off. Reporting a peer as verified when the write
   * failed is the single worst thing this UI can do: the entire trust model
   * hangs off the user believing they checked a fingerprint.
   */
  it("handleVerify surfaces a backend failure instead of reporting success", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_muted_conversations") return [];
      if (cmd === "verify_peer") throw new Error("key store is locked");
      return undefined;
    });

    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    await establishConnection();

    await user.click(screen.getByText("Verify"));

    // The rejection propagated, so the caller knows not to claim success.
    expect(screen.getByTestId("verify-error")).toHaveTextContent(/key store is locked/);
    // And the peer was NOT marked verified.
    expect(screen.getByTestId("peer-verified")).not.toHaveTextContent("true");
    expect(mockInvoke).toHaveBeenCalledWith("verify_peer", {
      peerKeyHex: "aabbcc",
    });
  });

  it("handleVerify marks the peer verified when the backend succeeds", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_muted_conversations") return [];
      if (cmd === "verify_peer") return undefined;
      return undefined;
    });

    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    await establishConnection();

    await user.click(screen.getByText("Verify"));

    // No error, and the trust indicator actually flips.
    expect(screen.getByTestId("verify-error")).toHaveTextContent("none");
    await waitFor(() =>
      expect(screen.getByTestId("peer-verified")).toHaveTextContent("true"),
    );
  });

  it("handleVerify refuses when there is no active peer", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_muted_conversations") return [];
      return undefined;
    });

    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    // With no connection there is nobody to verify. It must throw rather than
    // resolve, so the caller cannot mistake a no-op for a success.
    await user.click(screen.getByText("Verify"));
    expect(screen.getByTestId("verify-error")).toHaveTextContent(/No active peer/);
    expect(screen.getByTestId("peer-verified")).not.toHaveTextContent("true");
  });
});
