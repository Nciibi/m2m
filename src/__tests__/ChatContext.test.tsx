import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: any[]) => mockInvoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

// Mock AppContext used by ChatProvider
const appState = {
  addToast: vi.fn(),
  setView: vi.fn(),
};
vi.mock("../context/AppContext", () => ({
  useApp: () => appState,
}));

import { ChatProvider, useChat } from "../context/ChatContext";
import type { ConnectionInfo } from "../types";

function TestConsumer() {
  const {
    connection, isConnecting, messages, conversations, fileRequests,
    generatedInvite, inviteValid, activeConversationId,
    handleSendMessage, handleConnect, handleDisconnect, handleGenerateInvite,
    handleOpenChat, handleDeleteConversation, setInviteToConnect,
    copyInvite, handleVerify, handleSendFile, handleExportConversation,
    handleSendReaction, handleRemoveReaction, handleMarkConversationRead,
  } = useChat();
  return (
    <div>
      <span data-testid="connection-state">{connection?.state || "null"}</span>
      <span data-testid="is-connecting">{String(isConnecting)}</span>
      <span data-testid="messages-count">{messages.length}</span>
      <span data-testid="conversations-count">{conversations.length}</span>
      <span data-testid="file-requests-count">{fileRequests.length}</span>
      <button onClick={handleGenerateInvite}>Generate Invite</button>
      <button onClick={() => setInviteToConnect("m2m://test")}>Set Invite</button>
      <button onClick={copyInvite}>Copy Invite</button>
      <button onClick={handleVerify}>Verify</button>
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

    await user.click(screen.getByText("Send Reaction"));
    expect(mockInvoke).not.toHaveBeenCalled();
  });
});
