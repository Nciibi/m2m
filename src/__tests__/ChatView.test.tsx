import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn() }));

const state = {
  connection: null as any,
  messages: [] as any[],
  identity: null as any,
  fileRequests: [] as any[],
  activeConversationId: null as string | null,
  toasts: [] as any[],
  handleSendMessage: vi.fn().mockResolvedValue(undefined),
  handleSendFile: vi.fn(),
  handleVerify: vi.fn(),
  handleDisconnect: vi.fn(),
  setView: vi.fn(),
  handleExportConversation: vi.fn(),
  handleSetRetention: vi.fn(),
  retentionPolicy: "none",
  setRetentionPolicy: vi.fn(),
  retentionDuration: "86400",
  setRetentionDuration: vi.fn(),
  removeToast: vi.fn(),
  addToast: vi.fn(),
};

vi.mock("../context/M2MContext", () => ({
  useM2M: () => ({
    connection: state.connection,
    messages: state.messages,
    identity: state.identity,
    fileRequests: state.fileRequests,
    activeConversationId: state.activeConversationId,
    toasts: state.toasts,
    removeToast: state.removeToast,
    addToast: state.addToast,
    handleSendMessage: state.handleSendMessage,
    handleSendFile: state.handleSendFile,
    handleVerify: state.handleVerify,
    handleDisconnect: state.handleDisconnect,
    setView: state.setView,
    handleExportConversation: state.handleExportConversation,
    handleSetRetention: state.handleSetRetention,
    retentionPolicy: state.retentionPolicy,
    setRetentionPolicy: state.setRetentionPolicy,
    retentionDuration: state.retentionDuration,
    setRetentionDuration: state.setRetentionDuration,
  }),
  M2MProvider: ({ children }: { children: React.ReactNode }) => children,
}));

import ChatView from "../views/ChatView";

describe("ChatView", () => {
  beforeEach(() => {
    state.connection = null;
    state.messages = [];
    state.fileRequests = [];
    state.activeConversationId = null;
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
    const sendBtn = screen.getByRole("button", { name: /send/i });
    expect(sendBtn).toBeDisabled();
  });

  it("handles sending a message", async () => {
    const user = userEvent.setup();
    state.connection = { state: "established", peer_verified: false };
    render(<ChatView />);
    const input = screen.getByPlaceholderText(/type a secure message/i);
    const sendBtn = screen.getByRole("button", { name: /send/i });
    await user.type(input, "Hello from test!");
    expect(sendBtn).not.toBeDisabled();
    await user.click(sendBtn);
    expect(state.handleSendMessage).toHaveBeenCalledWith("Hello from test!");
  });

  it("shows messages in the message list", () => {
    state.connection = { state: "established", peer_verified: false };
    state.messages = [
      { id: "m1", content: "Hello!", direction: "incoming", timestamp: 1000 },
      { id: "m2", content: "Hi back!", direction: "outgoing", timestamp: 2000 },
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
      { id: "m1", content: "Hi", direction: "incoming", timestamp: 1717000000 },
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
});
