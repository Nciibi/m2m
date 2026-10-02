import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, screen, waitFor } from "@testing-library/react";
import { render } from "./setup";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import type { MockEventHandler } from "./tauriMock";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => mockInvoke(...args) }));
// Capture the registered event handlers so a test can simulate a peer
// connecting — the trust-anchor behaviour under test only applies once
// `connection.peer_key_hex` is set.
// NOTE: the Map is constructed inline rather than via a helper from
// ./tauriMock — `vi.hoisted` callbacks are hoisted above the import statements,
// so they cannot reference an imported *value* (a type-only import like
// `MockEventHandler` is erased and is fine).
const { eventHandlers } = vi.hoisted(() => ({
  eventHandlers: new Map<string, MockEventHandler>(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: MockEventHandler) => {
    eventHandlers.set(name, handler);
    return Promise.resolve(() => eventHandlers.delete(name));
  }),
}));

// Mock AppContext used by ChatProvider
const appState = {
  addToast: vi.fn(),
  setView: vi.fn(),
  // `ChatProvider` reads `identity?.public_key_hex` to decide which reaction
  // chips are the local user's, and clears messages on `m2m://vault-locked`.
  // A 64-char hex key, so `isPeerKeyHex`-shaped rather than a sentinel.
  identity: { public_key_hex: "a".repeat(64), fingerprint: "f", has_identity: true },
};
vi.mock("../context/AppContext", () => ({
  useApp: () => appState,
}));

import { ChatProvider, useChat } from "../context/ChatContext";

/** The emoji the reaction tests send, referenced by name to keep it readable. */
const REACTION = "\u{1F44D}";

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
      <span data-testid="messages-text">{messages.map((m) => m.content).join("|")}</span>
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
      {/* Wrapped, not passed directly: the handler now takes a conversationId,
          so `onClick={handleDeleteConversation}` would hand it a MouseEvent.
          That mismatch is exactly what the parameterless signature hid. */}
      <button onClick={() => handleDeleteConversation("conv-1")}>Delete Conv</button>
      <button onClick={() => handleOpenChat({ id: "c1", peer_key_hex: "abc", display_name: null, peer_display_name: null, last_message_at: null, last_message_preview: null, message_count: 0, is_online: false, auto_delete_at: null, retention_policy: "none", created_at: 0 })}>Open Chat</button>
      <button onClick={() => handleOpenChat({ id: "cb", peer_key_hex: "b".repeat(64), display_name: null, peer_display_name: null, last_message_at: null, last_message_preview: null, message_count: 0, is_online: true, auto_delete_at: null, retention_policy: "none", created_at: 0 })}>Open Peer B</button>
      <button onClick={() => handleSendReaction("msg-1", REACTION)}>Send Reaction</button>
      <button onClick={() => handleRemoveReaction("msg-1", REACTION)}>Remove Reaction</button>
      <button onClick={handleMarkConversationRead}>Mark Read</button>
    </div>
  );
}

/** Surfaces `messages[0].reactions` so a test can assert the optimistic shape. */
function ReactionProbe() {
  const { messages } = useChat();
  return (
    <span data-testid="reactors">{JSON.stringify(messages[0]?.reactions ?? {})}</span>
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
      reaction: REACTION,
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
      reaction: REACTION,
    });
  });

  // ─── Optimistic reactions use the REAL identity key, not a "self" sentinel ───
  //
  // The backend persists the reactor's Ed25519 key (`send_reaction` →
  // `upsert_reaction(..., &peer_key_hex, ...)`), so `load_messages` returns
  // `[<64-hex>]` and never `"self"`. The optimistic write used a `"self"`
  // sentinel, which meant the two halves disagreed the instant the page was
  // reloaded: the chip came back unhighlighted and a second click re-sent the
  // reaction instead of removing it. `events.ts` also validates every reactor
  // with `isPeerKeyHex`, so `"self"` was a value the app's own event boundary
  // would reject.

  it("optimistic reaction insert uses the local identity key, not a sentinel", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue([
      { id: "msg-1", content: "hi", direction: "received", timestamp: 1, reactions: {} },
    ]);
    render(
      <ChatProvider>
        <TestConsumer />
        <ReactionProbe />
      </ChatProvider>,
    );

    await user.click(screen.getByText("Open Chat"));
    mockInvoke.mockClear();
    mockInvoke.mockResolvedValue(undefined);
    await user.click(screen.getByText("Send Reaction"));

    // Assert the key is *present*, not merely that the probe string is non-empty
    // — `"{}"` is already non-empty, so a weaker assertion would pass before the
    // optimistic update had run at all.
    await waitFor(() =>
      expect(screen.getByTestId("reactors").textContent).toContain(REACTION),
    );
    const reactors = JSON.parse(screen.getByTestId("reactors").textContent || "{}") as Record<string, string[]>;
    expect(reactors[REACTION]).toEqual(["a".repeat(64)]);
    // And explicitly not the sentinel the old code wrote.
    expect(reactors[REACTION]).not.toContain("self");
  });

  it("failed reaction insert rolls back the optimistic key", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue([
      { id: "msg-1", content: "hi", direction: "received", timestamp: 1, reactions: {} },
    ]);
    render(
      <ChatProvider>
        <TestConsumer />
        <ReactionProbe />
      </ChatProvider>,
    );

    await user.click(screen.getByText("Open Chat"));
    mockInvoke.mockClear();
    // Reject only the reaction write.
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === "send_reaction" ? Promise.reject("nope") : Promise.resolve([]),
    );
    await user.click(screen.getByText("Send Reaction"));

    // The user must not be left believing a reaction was delivered.
    await waitFor(() =>
      expect(appState.addToast).toHaveBeenCalledWith(
        expect.stringContaining("Reaction failed"),
        "error",
      ),
    );
    expect(screen.getByTestId("reactors").textContent).toBe("{}");
  });

  it("handleMarkConversationRead calls mark_messages_read", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue([]); // default for load_messages
    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>,
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
        // A full 64-hex-char peer key: the payload validator rejects
        // anything shorter, which is the point — a malformed key must never
        // reach `setActiveConversationId` or a `load_messages` query.
        payload: {
          state: "established",
          peer_key_hex: "a".repeat(64),
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
      peerKeyHex: "a".repeat(64),
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

describe("ChatContext — inbound 1:1 messages", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    appState.addToast.mockClear();
  });

  /**
   * A payload shaped the way the backend actually emits a direct message.
   *
   * The critical detail is `sender_peer_key_hex: ""`. The Rust `ChatMessage`
   * documents that field as "Empty string for 1:1 messages (implicit from
   * conversation)" and `ChatMessage::new` defaults it to `String::new()`.
   *
   * A validator requiring a 64-char hex key rejected every one of these, so
   * this listener dropped 100% of 1:1 traffic while the whole suite stayed
   * green — no test at this layer ever exercised `m2m://message` at all.
   */
  function directMessage(over: Record<string, unknown> = {}) {
    return {
      peer_key_hex: "b".repeat(64),
      message: {
        id: "m1",
        content: "the real 1:1 message",
        direction: "received",
        timestamp: 1_700_000_000,
        read_at: null,
        edited_at: null,
        deleted: false,
        expires_at: null,
        reactions: {},
        sender_peer_key_hex: "",
        ...over,
      },
    };
  }

  async function mount() {
    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>,
    );
    await waitFor(() =>
      expect(eventHandlers.get("m2m://message")).toBeDefined(),
    );
  }

  /**
   * Open the conversation the inbound message belongs to.
   *
   * The listener now only appends to the transcript on screen, so a test that
   * fires `m2m://message` without opening a conversation is no longer a
   * realistic scenario — and the four tests below were passing *because* of the
   * cross-conversation contamination they never exercised.
   */
  async function openConversation() {
    // `TestConsumer`'s "Open Peer B" button opens the conversation whose peer
    // key matches the inbound fixtures below, so a message addressed to it is
    // the one that belongs on screen.
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_connection_state") {
        return Promise.resolve({ state: "established", peer_verified: false, peer_key_hex: "b".repeat(64) });
      }
      if (cmd === "list_conversations") return Promise.resolve([]);
      if (cmd === "load_messages") return Promise.resolve([]);
      return Promise.resolve(null);
    });
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /Open Peer B/i }));
    await waitFor(() =>
      expect(screen.getByTestId("connection-state")).toHaveTextContent("established"),
    );
    return user;
  }

  it("appends an inbound direct message with an empty sender key", async () => {
    await mount();
    await openConversation();
    act(() => {
      eventHandlers.get("m2m://message")?.({ payload: directMessage() });
    });
    expect(screen.getByTestId("messages-count")).toHaveTextContent("1");
    expect(screen.getByTestId("messages-text")).toHaveTextContent(
      "the real 1:1 message",
    );
  });

  it("appends a direct message whose sender key field is absent", async () => {
    await mount();
    await openConversation();
    const { sender_peer_key_hex: _omitted, ...message } = directMessage().message;
    act(() => {
      eventHandlers.get("m2m://message")?.({
        payload: { peer_key_hex: "b".repeat(64), message },
      });
    });
    expect(screen.getByTestId("messages-count")).toHaveTextContent("1");
  });

  it("still appends group messages that carry a real sender key", async () => {
    await mount();
    await openConversation();
    act(() => {
      eventHandlers.get("m2m://message")?.({
        payload: directMessage({
          sender_peer_key_hex: "c".repeat(64),
          content: "group message",
        }),
      });
    });
    expect(screen.getByTestId("messages-text")).toHaveTextContent("group message");
  });

  it("appends several messages in arrival order", async () => {
    await mount();
    await openConversation();
    act(() => {
      for (const n of [1, 2, 3]) {
        eventHandlers.get("m2m://message")?.({
          payload: directMessage({ id: `m${n}`, content: `msg ${n}` }),
        });
      }
    });
    expect(screen.getByTestId("messages-count")).toHaveTextContent("3");
    expect(screen.getByTestId("messages-text")).toHaveTextContent("msg 1|msg 2|msg 3");
  });

  it("does NOT append a message addressed to a different conversation", async () => {
    // The contamination bug: `m2m://message` appended to whatever conversation
    // was open, so reading source B while informant C writes put C's message
    // in B's transcript — C's sender label inside B's session banner, with
    // replies going to B. Six inbound tests existed and none of them opened a
    // different conversation first, which is why it was invisible.
    await mount();
    await openConversation();
    act(() => {
      eventHandlers.get("m2m://message")?.({
        payload: {
          ...directMessage({ content: "WRONG PEER" }),
          peer_key_hex: "d".repeat(64),
        },
      });
    });
    expect(screen.getByTestId("messages-count")).toHaveTextContent("0");
    expect(screen.getByTestId("messages-text")).not.toHaveTextContent("WRONG PEER");
  });

  it("still drops a malformed payload", async () => {
    await mount();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    act(() => {
      eventHandlers.get("m2m://message")?.({
        payload: directMessage({ content: { evil: true } }),
      });
    });
    expect(screen.getByTestId("messages-count")).toHaveTextContent("0");
    warn.mockRestore();
  });

  it("drops a payload whose peer key is not 64 hex chars", async () => {
    await mount();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    act(() => {
      eventHandlers.get("m2m://message")?.({
        payload: { ...directMessage(), peer_key_hex: "short" },
      });
    });
    expect(screen.getByTestId("messages-count")).toHaveTextContent("0");
    warn.mockRestore();
  });

  /**
   * Regression: the notification→navigate path was completely dead.
   *
   * `ChatContext` queued a `{peerKeyHex}` intent into a ref and bumped a
   * `forceNavRender` counter to trigger a second effect that drained it. That
   * effect's deps were `[setActiveConversation, setView, addToast]` — three
   * stable `useCallback`s — so it ran once, on mount, with the ref still null.
   * A counter bump schedules a re-render, and a `useEffect` with unchanged deps
   * does not re-run on one. Clicking the OS notification therefore focused the
   * window and never opened the conversation: no error, no log, no test failure.
   *
   * Asserted on `load_messages` rather than on rendered state, because opening
   * the conversation *is* that call — `handleOpenChat` is never invoked here, so
   * the drain is the only thing that can produce it.
   */
  it("opens the conversation when a message arrives for a peer that is not open", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_conversations") return Promise.resolve([]);
      if (cmd === "load_messages") return Promise.resolve([]);
      if (cmd === "get_muted_conversations") return Promise.resolve([]);
      if (cmd === "get_connection_state") {
        return Promise.resolve({ state: "established", peer_verified: false, peer_key_hex: "b".repeat(64) });
      }
      return Promise.resolve(null);
    });
    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>,
    );
    await waitFor(() =>
      expect(eventHandlers.get("m2m://message")).toBeDefined(),
    );
    // No conversation is open, so the notification branch is reachable. The
    // permission effect resolves asynchronously; let it land.
    await act(async () => { await Promise.resolve(); });

    mockInvoke.mockClear();
    act(() => {
      eventHandlers.get("m2m://message")?.({ payload: directMessage() });
    });

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("load_messages", { peerKeyHex: "b".repeat(64) }),
    );
  });
});
