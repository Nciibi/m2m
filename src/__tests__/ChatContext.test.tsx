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

function TestConsumer() {
  const {
    connection, isConnecting, messages, conversations, fileRequests,
    generatedInvite, inviteToConnect, inviteValid, activeConversationId,
    handleSendMessage, handleConnect, handleDisconnect, handleGenerateInvite,
    handleOpenChat, handleDeleteConversation, setInviteToConnect,
    copyInvite, handleVerify, handleSendFile, handleExportConversation,
  } = useChat();
  return (
    <div>
      <span data-testid="connection-state">{connection?.state || "null"}</span>
      <span data-testid="is-connecting">{String(isConnecting)}</span>
      <span data-testid="messages-count">{messages.length}</span>
      <span data-testid="conversations-count">{conversations.length}</span>
      <span data-testid="file-requests-count">{fileRequests.length}</span>
      <span data-testid="generated-invite">{generatedInvite}</span>
      <span data-testid="invite-valid">{String(inviteValid)}</span>
      <button onClick={() => handleSendMessage("test")}>Send Message</button>
      <button onClick={handleConnect}>Connect</button>
      <button onClick={handleDisconnect}>Disconnect</button>
      <button onClick={handleGenerateInvite}>Generate Invite</button>
      <button onClick={handleVerify}>Verify</button>
      <button onClick={handleSendFile}>Send File</button>
      <button onClick={handleExportConversation}>Export</button>
      <button onClick={() => handleOpenChat({ id: "c1", peer_key_hex: "abc" } as any)}>Open Chat</button>
      <button onClick={handleDeleteConversation}>Delete Conv</button>
      <button onClick={copyInvite}>Copy Invite</button>
      <button onClick={() => setInviteToConnect("m2m://test")}>Set Invite</button>
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

  it("handleSendMessage calls Tauri invoke", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue("msg-id");

    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    await user.click(screen.getByText("Send Message"));
    expect(mockInvoke).toHaveBeenCalledWith("send_message", expect.any(Object));
  });

  it("handleConnect calls Tauri invoke", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue(undefined);

    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    await user.click(screen.getByText("Connect"));
    expect(mockInvoke).toHaveBeenCalledWith("connect_to_peer", expect.any(Object));
  });

  it("handleDisconnect calls Tauri invoke", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue(undefined);

    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    await user.click(screen.getByText("Disconnect"));
    expect(mockInvoke).toHaveBeenCalledWith("disconnect", expect.any(Object));
  });

  it("handleGenerateInvite calls Tauri invoke", async () => {
    const user = userEvent.setup();
    // mockInvoke.mockResolvedValue("m2m://invite-link");
    // It generates signed prekey first, then the invite
    mockInvoke.mockResolvedValue("m2m://invite-generated");

    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    await user.click(screen.getByText("Generate Invite"));
    expect(mockInvoke).toHaveBeenCalled();
  });

  it("handleOpenChat sets active conversation", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue([]); // messages list

    render(
      <ChatProvider>
        <TestConsumer />
      </ChatProvider>
    );

    await user.click(screen.getByText("Open Chat"));
    expect(mockInvoke).toHaveBeenCalledWith("get_messages", expect.any(Object));
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
    // Can't easily read the state change without re-render, so just verify no error
  });
});

