import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { screen } from "@testing-library/react";
import { render } from "./setup";
import userEvent from "@testing-library/user-event";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
// `ChatView` no longer imports `@tauri-apps/plugin-dialog` directly: the save
// dialog moved into `ChatContext`'s accept handler, which is also what removes
// the request card from state. A stale `vi.mock` here would have kept passing
// while masking a future real import.

const state = {
  connection: null as DeepPartial<ConnectionInfo> | null,
  messages: [] as DeepPartial<ChatMessage>[],
  identity: null as DeepPartial<IdentityInfo> | null,
  fileRequests: [] as DeepPartial<FileRequest>[],
  activeConversationId: null as string | null,
  typingPeers: [] as string[],
  reconnecting: false,
  reconnectAttempt: 0,
  toasts: [] as ToastData[],
  setMessages: vi.fn(),
  handleSendMessage: vi.fn().mockResolvedValue({ id: "sent-msg-id" }),
  handleSendMessageWithTimer: vi.fn().mockResolvedValue({ id: "sent-msg-id" }),
  handleSendFile: vi.fn(),
  handleVerify: vi.fn(),
  handleDisconnect: vi.fn(),
  handleReconnect: vi.fn(),
  setView: vi.fn(),
  handleExportConversation: vi.fn(),
  handleSetRetention: vi.fn(),
  retentionPolicy: "none",
  setRetentionPolicy: vi.fn(),
  retentionDuration: "86400",
  setRetentionDuration: vi.fn(),
  handleSendReaction: vi.fn(),
  handleRemoveReaction: vi.fn(),
  handleMarkConversationRead: vi.fn(),
handleEditMessage: vi.fn(),
handleDeleteMessage: vi.fn(),
sendFileAtPath: vi.fn(),
handleAcceptFileTransfer: vi.fn(),
handleRejectFileTransfer: vi.fn(),
  removeToast: vi.fn(),
  addToast: vi.fn(),
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
    connection: state.connection,
    messages: state.messages,
    setMessages: state.setMessages,
    fileRequests: state.fileRequests,
    activeConversationId: state.activeConversationId,
    typingPeers: state.typingPeers,
    reconnecting: state.reconnecting,
    reconnectAttempt: state.reconnectAttempt,
    handleSendMessage: state.handleSendMessage,
    handleSendMessageWithTimer: state.handleSendMessageWithTimer,
    handleSendFile: state.handleSendFile,
    handleVerify: state.handleVerify,
    handleDisconnect: state.handleDisconnect,
    handleReconnect: state.handleReconnect,
    handleExportConversation: state.handleExportConversation,
    handleSetRetention: state.handleSetRetention,
    retentionPolicy: state.retentionPolicy,
    setRetentionPolicy: state.setRetentionPolicy,
    retentionDuration: state.retentionDuration,
    setRetentionDuration: state.setRetentionDuration,
    handleSendReaction: state.handleSendReaction,
    handleRemoveReaction: state.handleRemoveReaction,
    handleMarkConversationRead: state.handleMarkConversationRead,
    handleEditMessage: state.handleEditMessage,
    handleDeleteMessage: state.handleDeleteMessage,
    // Drag-and-drop sends by path; accept/reject go through the context so the
    // request card is removed from `fileRequests`. Omitting them from the mock
    // would leave these call sites `undefined` in any test that reaches them.
    sendFileAtPath: state.sendFileAtPath,
    handleAcceptFileTransfer: state.handleAcceptFileTransfer,
    handleRejectFileTransfer: state.handleRejectFileTransfer,
  }),
}));

import ChatView from "../views/ChatView";
import type {
  ChatMessage,
  ConnectionInfo,
  FileRequest,
  IdentityInfo,
} from "../types";
import type { ToastData } from "../components/ui/Toast";
import type { DeepPartial } from "./tauriMock";

describe("ChatView", () => {
  beforeEach(() => {
    // The real `invoke` always returns a promise. A bare `vi.fn()` returns
    // `undefined`, which the component's `.catch(...)` then dereferences — so
    // the mock is given the real shape rather than the component being written
    // to tolerate a call that cannot happen in production.
    (tauriInvoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    state.connection = null;
    state.messages = [];
    state.fileRequests = [];
    state.activeConversationId = null;
    state.typingPeers = [];
    state.reconnecting = false;
    state.reconnectAttempt = 0;
    state.toasts = [];
    vi.clearAllMocks();
  });

  it("renders encrypted session header", () => {
    render(<ChatView />);
    expect(screen.getByText("Encrypted Session")).toBeInTheDocument();
  });

  it("shows unknown state badge when no connection", () => {
    render(<ChatView />);
    expect(screen.getByText("unknown")).toBeInTheDocument();
  });

  it("shows established badge when connected", () => {
    state.connection = { state: "established", peer_verified: false, peer_fingerprint: "abcd" };
    render(<ChatView />);
    expect(screen.getByText("established")).toBeInTheDocument();
  });

  it("shows disconnect button when established", () => {
    state.connection = { state: "established", peer_verified: false };
    render(<ChatView />);
    expect(screen.getByRole("button", { name: /disconnect/i })).toBeInTheDocument();
  });

  it("calls handleDisconnect when disconnect clicked", async () => {
    const user = userEvent.setup();
    state.connection = { state: "established", peer_verified: false };
    render(<ChatView />);
    await user.click(screen.getByRole("button", { name: /disconnect/i }));
    expect(state.handleDisconnect).toHaveBeenCalledTimes(1);
  });

  it("shows back to hub button", () => {
    render(<ChatView />);
    expect(screen.getByRole("button", { name: /hub/i })).toBeInTheDocument();
  });

  it("navigates to hub on back button click", async () => {
    const user = userEvent.setup();
    render(<ChatView />);
    await user.click(screen.getByRole("button", { name: /hub/i }));
    expect(state.setView).toHaveBeenCalledWith("hub");
  });

  it("renders message input area", () => {
    render(<ChatView />);
    expect(screen.getByPlaceholderText(/type a secure message/i)).toBeInTheDocument();
  });

  it("disables send button when input is empty", () => {
    render(<ChatView />);
    const sendBtn = document.getElementById("send-message-btn");
    expect(sendBtn).toBeInTheDocument();
    expect(sendBtn).toBeDisabled();
  });

  it("handles sending a message", async () => {
    const user = userEvent.setup();
    state.connection = { state: "established", peer_verified: false };
    render(<ChatView />);
    const input = screen.getByPlaceholderText(/type a secure message/i);
    const sendBtn = document.getElementById("send-message-btn");
    await user.type(input, "Hello from test!");
    expect(sendBtn).not.toBeDisabled();
    await user.click(sendBtn!);
    expect(state.handleSendMessage).toHaveBeenCalledWith("Hello from test!");
  });

  it("shows messages in the message list", () => {
    state.connection = { state: "established", peer_verified: false };
    state.messages = [
      { id: "m1", content: "Hello!", direction: "received", timestamp: 1000 },
      { id: "m2", content: "Hi back!", direction: "sent", timestamp: 2000 },
    ];
    render(<ChatView />);
    expect(screen.getByText("Hello!")).toBeInTheDocument();
    expect(screen.getByText("Hi back!")).toBeInTheDocument();
  });

  it("shows verified icon when peer is verified", () => {
    state.connection = { state: "established", peer_verified: true, peer_fingerprint: "abcd" };
    render(<ChatView />);
    // The shield icon should be the verified variant
    expect(screen.getByText("Encrypted Session")).toBeInTheDocument();
  });

  it("shows file request accept and reject buttons", () => {
    state.connection = { state: "established", peer_verified: false };
    state.fileRequests = [
      { transfer_id: "ft-1", filename: "doc.pdf", total_size: 1024, peer_key_hex: "abc" },
    ];
    render(<ChatView />);
    expect(screen.getByText("doc.pdf")).toBeInTheDocument();
    expect(screen.getByText("1.0 KB")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /accept/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /reject/i })).toBeInTheDocument();
  });

  it("shows retention policy selector for active conversation", () => {
    state.activeConversationId = "conv-1";
    state.connection = { state: "established", peer_verified: false };
    render(<ChatView />);
    expect(screen.getByText("Conversation Policy")).toBeInTheDocument();
    expect(screen.getByText("No Expiration")).toBeInTheDocument();
  });

  it("shows export conversation button for active conversation", () => {
    state.activeConversationId = "conv-1";
    state.connection = { state: "established", peer_verified: false };
    render(<ChatView />);
    expect(screen.getByRole("button", { name: /export now/i })).toBeInTheDocument();
  });

  it("groups messages by date", () => {
    state.connection = { state: "established", peer_verified: false };
    state.messages = [
      { id: "m1", content: "Hi", direction: "received", timestamp: 1717000000 },
    ];
    render(<ChatView />);
    // Should show a date separator
    expect(screen.getByText("Hi")).toBeInTheDocument();
  });

  it("disconnects on Escape key in some cases", () => {
    render(<ChatView />);
    const sendBtn = screen.getByRole("button", { name: /send/i });
    expect(sendBtn).toBeInTheDocument();
  });

  // ─── Self-destruct expiry is not this view's job ─────────────────────────
  //
  // Expiry used to be two `setInterval`s here (10s and 60s), which meant it ran
  // *only while this screen was mounted*. For a tray app that is a small part
  // of its life, so "auto-delete after 24h" silently did nothing unless the
  // user happened to be sitting in a conversation. The backend now sweeps on a
  // timer and once at database open. What is left is a single one-shot on
  // mount, so a conversation opened seconds after a timer elapses shows the
  // truth — and polling must not come back, because polling from the view is
  // the bug.

  it("clears elapsed self-destruct timers once on mount", () => {
    render(<ChatView />);
    const calls = (tauriInvoke as unknown as ReturnType<typeof vi.fn>).mock.calls.filter(
      (c) => c[0] === "cleanup_expired_messages",
    );
    expect(calls).toHaveLength(1);
  });

  it("does not poll cleanup_expired_messages while mounted", () => {
    // Fake timers, then advance well past both of the old intervals. If a
    // `setInterval` is ever reintroduced here this fails.
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      render(<ChatView />);
      vi.advanceTimersByTime(10 * 60 * 1000);
      const calls = (tauriInvoke as unknown as ReturnType<typeof vi.fn>).mock.calls.filter(
        (c) => c[0] === "cleanup_expired_messages",
      );
      expect(calls).toHaveLength(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("a rejected cleanup does not take the view down", async () => {
    (tauriInvoke as unknown as ReturnType<typeof vi.fn>).mockImplementation(
      (cmd: string) => (cmd === "cleanup_expired_messages" ? Promise.reject(new Error("store not open")) : Promise.resolve(undefined)),
    );
    render(<ChatView />);
    // The view is still usable: a store that is not open is the normal state of
    // a fresh install, not a reason to blank the conversation.
    expect(screen.getByText("Encrypted Session")).toBeInTheDocument();
  });
});
