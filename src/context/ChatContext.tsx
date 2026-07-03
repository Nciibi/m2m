import {
  createContext, useContext, useState, useEffect, useCallback, ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useApp } from "./AppContext";
import type {
  ConnectionInfo, ChatMessage, FileRequest, ConversationEntry,
} from "../types";

interface ChatContextValue {
  connection: ConnectionInfo | null;
  isConnecting: boolean;
  reconnecting: boolean;
  reconnectAttempt: number;
  messages: ChatMessage[];
  setMessages: React.Dispatch<React.SetStateAction<ChatMessage[]>>;
  fileRequests: FileRequest[];
  conversations: ConversationEntry[];
  typingPeers: string[];
  activeConversationId: string | null;
  inviteToConnect: string;
  setInviteToConnect: (v: string) => void;
  inviteValid: boolean;
  namingMyName: string;
  setNamingMyName: (v: string) => void;
  namingTheirName: string;
  setNamingTheirName: (v: string) => void;
  generatedInvite: string;
  retentionPolicy: string;
  setRetentionPolicy: (v: string) => void;
  retentionDuration: string;
  setRetentionDuration: (v: string) => void;
  handleSendMessage: (content: string) => Promise<ChatMessage>;
  handleVerify: () => Promise<void>;
  handleDisconnect: () => Promise<void>;
  handleReconnect: () => Promise<void>;
  handleSendFile: () => Promise<void>;
  handleExportConversation: () => Promise<void>;
  handleSetRetention: (policy: string, durationSecs: number | null) => Promise<void>;
  handleGenerateInvite: () => Promise<void>;
  copyInvite: () => void;
  handleConnect: () => Promise<void>;
  handleOpenChat: (conv: ConversationEntry) => Promise<void>;
  handleDeleteConversation: () => void;
  // Reactions & Read Receipts
  handleSendReaction: (messageId: string, reaction: string) => Promise<void>;
  handleRemoveReaction: (messageId: string, reaction: string) => Promise<void>;
  handleMarkConversationRead: () => Promise<void>;
  // Self-destruct, Edit, Delete
  handleSendMessageWithTimer: (content: string, disappearAfter?: number) => Promise<ChatMessage>;
  handleEditMessage: (messageId: string, newContent: string) => Promise<void>;
  handleDeleteMessage: (messageId: string) => Promise<void>;
  // Mute
  mutedConversations: string[];
  handleMuteConversation: (peerKeyHex: string) => Promise<void>;
  handleUnmuteConversation: (peerKeyHex: string) => Promise<void>;
  // File Transfer
  handleAcceptFileTransfer: (transferId: string) => Promise<void>;
  handleRejectFileTransfer: (transferId: string) => Promise<void>;
}

const ChatContext = createContext<ChatContextValue | null>(null);

export function useChat(): ChatContextValue {
  const ctx = useContext(ChatContext);
  if (!ctx) throw new Error("useChat() must be used within <ChatProvider>");
  return ctx;
}

export function ChatProvider({ children }: { children: ReactNode }) {
  const { addToast, setView } = useApp();

  // ─── State ───
  const [connection, setConnection] = useState<ConnectionInfo | null>(null);
  const [isConnecting, setIsConnecting] = useState(false);
  const [reconnecting, setReconnecting] = useState(false);
  const [reconnectAttempt, setReconnectAttempt] = useState(0);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [fileRequests, setFileRequests] = useState<FileRequest[]>([]);
  const [transfers, setTransfers] = useState<TransferProgress[]>([]);
  const [typingPeers, setTypingPeers] = useState<string[]>([]);
  const [conversations, setConversations] = useState<ConversationEntry[]>([]);
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null);
  const [inviteToConnect, setInviteToConnect] = useState("");
  const [inviteValid, setInviteValid] = useState(false);
  const [namingMyName, setNamingMyName] = useState("");
  const [namingTheirName, setNamingTheirName] = useState("");
  const [generatedInvite, setGeneratedInvite] = useState("");
  const [retentionPolicy, setRetentionPolicy] = useState("none");
  const [retentionDuration, setRetentionDuration] = useState<string>("86400");

  const loadConversations = useCallback(async () => {
    try {
      setConversations(await invoke<ConversationEntry[]>("list_conversations"));
    } catch { /* noop */ }
  }, []);

  // ─── Handlers ───

  const handleSendMessage = useCallback(async (content: string): Promise<ChatMessage> => {
    if (!connection?.peer_key_hex) throw new Error("Not connected");
    const msg = await invoke<ChatMessage>("send_message", {
      peerKeyHex: connection.peer_key_hex,
      content,
    });
    setMessages((prev) => [...prev, msg]);
    return msg;
  }, [connection?.peer_key_hex]);

  const handleVerify = useCallback(async () => {
    if (!connection?.peer_key_hex) return;
    try {
      await invoke("verify_peer", { peerKeyHex: connection.peer_key_hex });
      setConnection({ ...connection, peer_verified: true });
    } catch { /* noop */ }
  }, [connection]);

  const handleDisconnect = useCallback(async () => {
    if (!connection?.peer_key_hex) return;
    try {
      await invoke("disconnect_peer", { peerKeyHex: connection.peer_key_hex });
      setView("hub");
      setConnection(null);
      setMessages([]);
    } catch { /* noop */ }
  }, [connection?.peer_key_hex, setView]);

  const handleSendFile = useCallback(async () => {
    if (!connection?.peer_key_hex) return;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({ multiple: false, title: "Select file to send" });
      if (!selected) return;
      const filePath = typeof selected === "string" ? selected : selected;
      await invoke("send_file", { peerKeyHex: connection.peer_key_hex, filePath });
      const filename = filePath.split(/[\\/]/).pop() || "file";
      setMessages((prev) => [...prev, {
        id: Date.now().toString(),
        content: `File request sent: ${filename}`,
        direction: "sent",
        timestamp: Math.floor(Date.now() / 1000),
        read_at: null,
        edited_at: null,
        deleted: false,
        expires_at: null,
        reactions: {},
        sender_peer_key_hex: "",
      } as ChatMessage]);
    } catch (e) {
      addToast("Failed to send file: " + e, "error");
    }
  }, [connection?.peer_key_hex, addToast]);

  const handleAcceptFileTransfer = useCallback(async (transferId: string) => {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const savePath = await save({ title: "Save incoming file" });
      if (!savePath) return;
      await invoke("accept_file_transfer", { transferId });
      setFileRequests((prev) => prev.filter((r) => r.transfer_id !== transferId));
      addToast("Downloading file...", "info");
    } catch (e) {
      addToast("Failed to accept transfer: " + e, "error");
    }
  }, [addToast]);

  const handleRejectFileTransfer = useCallback(async (transferId: string) => {
    try {
      await invoke("reject_file_transfer", { transferId });
      setFileRequests((prev) => prev.filter((r) => r.transfer_id !== transferId));
      addToast("File transfer rejected", "info");
    } catch (e) {
      addToast("Failed to reject transfer: " + e, "error");
    }
  }, [addToast]);

  const handleExportConversation = useCallback(async () => {
    if (!activeConversationId) return;
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const savePath = await save({
        title: "Export Conversation",
        defaultPath: `export_${activeConversationId}.json`,
      });
      if (savePath) {
        await invoke("export_conversation", { conversationId: activeConversationId, exportPath: savePath });
        addToast("Exported successfully", "success");
      }
    } catch (e) {
      addToast("Export failed: " + e, "error");
    }
  }, [activeConversationId, addToast]);

  const handleSetRetention = useCallback(async (policy: string, durationSecs: number | null) => {
    if (!activeConversationId) return;
    try {
      await invoke("set_conversation_retention", { conversationId: activeConversationId, policy, durationSecs });
    } catch { /* noop */ }
  }, [activeConversationId]);

  const handleGenerateInvite = useCallback(async () => {
    try {
      await invoke("start_listening", { address: "0.0.0.0:0" });
      const address = await invoke<string>("get_listen_address");
      const invite = await invoke<string>("create_invite", { address, validityMinutes: 60, oneTime: true });
      setGeneratedInvite(invite);
    } catch (e) {
      addToast(String(e), "error", 6000);
    }
  }, [addToast]);

  const copyInvite = useCallback(() => {
    navigator.clipboard.writeText(generatedInvite);
  }, [generatedInvite]);

  const handleConnect = useCallback(async () => {
    if (!inviteToConnect) return;
    setIsConnecting(true);
    try {
      const info = await invoke<ConnectionInfo>("connect_to_peer", { inviteStr: inviteToConnect });
      setConnection(info);
      setActiveConversationId(info.peer_key_hex || null);
      if (info.peer_key_hex && (namingMyName || namingTheirName)) {
        await invoke("send_conversation_names", {
          peerKeyHex: info.peer_key_hex, myName: namingMyName, theirName: namingTheirName,
        }).catch(() => {});
      }
      setView("chat");
      try {
        setMessages(await invoke<ChatMessage[]>("load_messages", { peerKeyHex: info.peer_key_hex }));
      } catch { /* noop */ }
    } catch (e) {
      addToast("Connection failed: " + e, "error");
    } finally {
      setIsConnecting(false);
    }
  }, [inviteToConnect, namingMyName, namingTheirName, addToast, setView]);

  const handleOpenChat = useCallback(async (conv: ConversationEntry) => {
    setActiveConversationId(conv.peer_key_hex);
    setRetentionPolicy(conv.retention_policy || "none");
    setView("chat");
    setConnection({
      state: conv.is_online ? "established" : "disconnected",
      peer_fingerprint: null,
      peer_verified: true,
      peer_key_hex: conv.peer_key_hex,
    });
    try {
      setMessages(await invoke<ChatMessage[]>("load_messages", { peerKeyHex: conv.peer_key_hex }));
    } catch { /* noop */ }
    // Mark messages as read when opening a conversation
    try {
      await invoke("mark_messages_read", { conversationId: conv.peer_key_hex });
    } catch { /* noop */ }
    // Refresh conversation list to update unread counts
    loadConversations();
  }, [setView, loadConversations]);

  const handleDeleteConversation = useCallback(() => {
    loadConversations();
  }, [loadConversations]);

  // ─── Reaction handlers ───

  const handleSendReaction = useCallback(async (messageId: string, reaction: string) => {
    if (!connection?.peer_key_hex) return;
    try {
      await invoke("send_reaction", { peerKeyHex: connection.peer_key_hex, messageId, reaction });
      // Optimistically update UI
      setMessages((prev) => prev.map((m) => {
        if (m.id !== messageId) return m;
        const reactions = { ...m.reactions };
        const reactors = reactions[reaction] || [];
        if (!reactors.includes("self")) {
          reactions[reaction] = [...reactors, "self"];
        }
        return { ...m, reactions };
      }));
    } catch { /* noop */ }
  }, [connection?.peer_key_hex]);

  const handleRemoveReaction = useCallback(async (messageId: string, reaction: string) => {
    if (!connection?.peer_key_hex) return;
    try {
      await invoke("remove_reaction", { peerKeyHex: connection.peer_key_hex, messageId, reaction });
      // Optimistically update UI
      setMessages((prev) => prev.map((m) => {
        if (m.id !== messageId) return m;
        const reactions = { ...m.reactions };
        const reactors = (reactions[reaction] || []).filter((r: string) => r !== "self");
        if (reactors.length === 0) {
          delete reactions[reaction];
        } else {
          reactions[reaction] = reactors;
        }
        return { ...m, reactions };
      }));
    } catch { /* noop */ }
  }, [connection?.peer_key_hex]);

  const handleReconnect = useCallback(async () => {
    if (!connection?.peer_key_hex) return;
    setReconnecting(true);
    setReconnectAttempt(1);
    try {
      const info = await invoke<ConnectionInfo>("attempt_reconnect", { peerKeyHex: connection.peer_key_hex });
      setConnection(info);
    } catch (e) {
      setReconnecting(false);
      setReconnectAttempt(0);
    }
  }, [connection]);

  const handleMarkConversationRead = useCallback(async () => {
    if (!activeConversationId) return;
    try {
      await invoke("mark_messages_read", { conversationId: activeConversationId });
      setMessages((prev) => prev.map((m) => {
        if (m.direction === "received" && m.read_at === null) {
          return { ...m, read_at: Math.floor(Date.now() / 1000) };
        }
        return m;
      }));
    } catch { /* noop */ }
  }, [activeConversationId]);

  // ─── Self-destruct, Edit, Delete handlers ───

  const handleSendMessageWithTimer = useCallback(async (content: string, disappearAfter?: number): Promise<ChatMessage> => {
    if (!connection?.peer_key_hex) throw new Error("Not connected");
    const msg = await invoke<ChatMessage>("send_message_with_timer", {
      peerKeyHex: connection.peer_key_hex,
      content,
      disappearAfter: disappearAfter ?? null,
    });
    setMessages((prev) => [...prev, msg]);
    return msg;
  }, [connection?.peer_key_hex]);

  const handleEditMessage = useCallback(async (messageId: string, newContent: string) => {
    if (!connection?.peer_key_hex) return;
    try {
      const updated = await invoke<ChatMessage>("edit_message", {
        peerKeyHex: connection.peer_key_hex,
        messageId,
        newContent,
      });
      setMessages((prev) => prev.map((m) => m.id === messageId ? updated : m));
    } catch (e) {
      addToast("Edit failed: " + e, "error");
    }
  }, [connection?.peer_key_hex, addToast]);

  const handleDeleteMessage = useCallback(async (messageId: string) => {
    if (!connection?.peer_key_hex) return;
    try {
      await invoke("delete_message", {
        peerKeyHex: connection.peer_key_hex,
        messageId,
      });
      // Optimistic update — mark as deleted immediately
      setMessages((prev) => prev.map((m) =>
        m.id === messageId ? { ...m, deleted: true, content: "[deleted]" } : m
      ));
    } catch (e) {
      addToast("Delete failed: " + e, "error");
    }
  }, [connection?.peer_key_hex, addToast]);

  // ─── Invite validation effect ───
  useEffect(() => {
    if (inviteToConnect.length > 30) {
      invoke<any>("validate_invite", { inviteStr: inviteToConnect })
        .then((info) => { if (info.valid) setInviteValid(true); })
        .catch(() => setInviteValid(false));
    } else {
      setInviteValid(false);
    }
  }, [inviteToConnect]);

  // ─── View switch: load conversations when entering hub ───
  const { view } = useApp();
  useEffect(() => {
    if (view === "hub") loadConversations();
  }, [view, loadConversations]);

  // ─── Notification permission + muted conversations ───
  const [notifPermission, setNotifPermission] = useState(false);
  const [mutedConversations, setMutedConversations] = useState<string[]>([]);

  const loadMutedConversations = useCallback(async () => {
    try { setMutedConversations(await invoke<string[]>("get_muted_conversations")); } catch { /* noop */ }
  }, []);

  useEffect(() => {
    (async () => {
      const { isPermissionGranted, requestPermission } = await import("@tauri-apps/plugin-notification");
      let granted = await isPermissionGranted();
      if (!granted) { const result = await requestPermission(); granted = result === "granted"; }
      setNotifPermission(granted);
    })();
  }, []);

  useEffect(() => { loadMutedConversations(); }, [loadMutedConversations]);

  const handleMuteConversation = useCallback(async (peerKeyHex: string) => {
    try { await invoke("mute_conversation", { peerKeyHex }); await loadMutedConversations(); } catch { /* noop */ }
  }, [loadMutedConversations]);

  const handleUnmuteConversation = useCallback(async (peerKeyHex: string) => {
    try { await invoke("unmute_conversation", { peerKeyHex }); await loadMutedConversations(); } catch { /* noop */ }
  }, [loadMutedConversations]);

  // ─── Tauri event listeners ───
  useEffect(() => {
    const unlistenMsg = listen<any>("m2m://message", (event) => {
      setMessages((prev) => [...prev, event.payload.message]);

      // Send native OS notification if:
      // 1. Notification permission granted
      // 2. Not currently viewing this conversation
      // 3. Conversation is not muted
      const peerKeyHex: string = event.payload.peer_key_hex;
      if (notifPermission && peerKeyHex !== activeConversationId && !mutedConversations.includes(peerKeyHex)) {
        const peerFingerprint = event.payload.peer_fingerprint ?? event.payload.message?.peer_fingerprint;
        const displayName = peerFingerprint
          ? peerFingerprint.substring(0, 8) + "…"
          : peerKeyHex.substring(0, 8) + "…";
        import("@tauri-apps/plugin-notification").then(({ sendNotification, isPermissionGranted: _i }) => {
          sendNotification({
            title: "M2M",
            body: `New message from ${displayName}`,
            group: peerKeyHex,
          });
        });
        // Clicking the notification opens the conversation
        const handleNotifClick = () => {
          if (peerKeyHex !== activeConversationId) {
            setActiveConversationId(peerKeyHex);
            setView("chat");
            invoke<ChatMessage[]>("load_messages", { peerKeyHex }).then(setMessages).catch(() => {});
          }
        };
        // Bind a one-time click handler if the notification is actionable
        document.addEventListener("visibilitychange", handleNotifClick, { once: true });
      }
    });

    const unlistenConn = listen<any>("m2m://connection", async (event) => {
      const stateStr = event.payload.state;
      setConnection({
        state: stateStr,
        peer_fingerprint: event.payload.peer_fingerprint,
        peer_verified: event.payload.peer_verified ?? false,
        peer_key_hex: event.payload.peer_key_hex,
      });
      if (stateStr === "established") {
        setReconnecting(false);
        setReconnectAttempt(0);
        setActiveConversationId(event.payload.peer_key_hex);
        setView("chat");
        try {
          setMessages(await invoke<ChatMessage[]>("load_messages", { peerKeyHex: event.payload.peer_key_hex }));
        } catch { /* noop */ }
      } else if (stateStr === "disconnected") {
        // For verified peers, stay on ChatView so user can attempt reconnect.
        // For unverified peers, go back to hub (no reconnect possible).
        if (!event.payload.peer_verified) {
          setView("hub");
          setConnection(null);
          setMessages([]);
          setActiveConversationId(null);
        }
      }
      try { setConversations(await invoke<ConversationEntry[]>("list_conversations")); } catch { /* noop */ }
    });

    const unlistenConvMeta = listen<any>("m2m://conversation-meta", async () => {
      try { setConversations(await invoke<ConversationEntry[]>("list_conversations")); } catch { /* noop */ }
    });

    const unlistenFileReq = listen<any>("m2m://file-request", (event) => {
      setFileRequests((prev) => [...prev, event.payload]);
    });

    const unlistenFileComp = listen<any>("m2m://file-complete", () => {});

    const unlistenReaction = listen<any>("m2m://reaction", (event) => {
      const { message_id, reaction, peer_key_hex, remove } = event.payload;
      // Only apply if this reaction is for a message in the current conversation
      setMessages((prev) => prev.map((m) => {
        if (m.id !== message_id) return m;
        const reactions = { ...m.reactions };
        if (remove) {
          const reactors = (reactions[reaction] || []).filter((r: string) => r !== peer_key_hex);
          if (reactors.length === 0) {
            delete reactions[reaction];
          } else {
            reactions[reaction] = reactors;
          }
        } else {
          const reactors = reactions[reaction] || [];
          if (!reactors.includes(peer_key_hex)) {
            reactions[reaction] = [...reactors, peer_key_hex];
          }
        }
        return { ...m, reactions };
      }));
    });

    const unlistenEdit = listen<any>("m2m://edit", (event) => {
      const { message_id, new_content, edited_at, peer_key_hex: _peer } = event.payload;
      setMessages((prev) => prev.map((m) =>
        m.id === message_id
          ? { ...m, content: new_content, edited_at }
          : m
      ));
    });

    const unlistenReconnectAttempt = listen<any>("m2m://reconnect-attempt", (event) => {
      const { state: reconnectState, attempt } = event.payload;
      if (reconnectState === "attempting") {
        setReconnecting(true);
        setReconnectAttempt(attempt);
      } else if (reconnectState === "success") {
        setReconnecting(false);
        setReconnectAttempt(0);
      } else if (reconnectState === "failed") {
        setReconnecting(false);
        setReconnectAttempt(0);
      }
    });

    const unlistenDelete = listen<any>("m2m://delete", (event) => {
      const { message_id } = event.payload;
      setMessages((prev) => prev.map((m) =>
        m.id === message_id
          ? { ...m, deleted: true, content: "[deleted]" }
          : m
      ));
    });

    const unlistenTyping = listen<any>("m2m://typing", (event) => {
      const { peer_key_hex: typingPeer, typing } = event.payload;
      if (typing) {
        setTypingPeers((prev: string[]) => prev.includes(typingPeer) ? prev : [...prev, typingPeer]);
      } else {
        setTypingPeers((prev: string[]) => prev.filter((p: string) => p !== typingPeer));
      }
    });

    return () => {
      unlistenMsg.then((f) => f());
      unlistenConn.then((f) => f());
      unlistenFileReq.then((f) => f());
      unlistenFileComp.then((f) => f());
      unlistenConvMeta.then((f) => f());
      unlistenReaction.then((f) => f());
      unlistenEdit.then((f) => f());
      unlistenReconnectAttempt.then((f) => f());
      unlistenDelete.then((f) => f());
      unlistenTyping.then((f) => f());
    };
  }, [setView, notifPermission, activeConversationId, mutedConversations]);

  return (
    <ChatContext.Provider value={{
      connection, isConnecting, reconnecting, reconnectAttempt, messages, setMessages, fileRequests,
      conversations, activeConversationId, typingPeers,
      inviteToConnect, setInviteToConnect, inviteValid,
      namingMyName, setNamingMyName, namingTheirName, setNamingTheirName,
      generatedInvite,
      retentionPolicy, setRetentionPolicy, retentionDuration, setRetentionDuration,
      handleSendMessage, handleVerify, handleDisconnect, handleReconnect, handleSendFile,
      handleExportConversation, handleSetRetention,
      handleGenerateInvite, copyInvite, handleConnect, handleOpenChat,
      handleDeleteConversation,
      handleSendReaction, handleRemoveReaction, handleMarkConversationRead,
      handleSendMessageWithTimer, handleEditMessage, handleDeleteMessage,
      mutedConversations, handleMuteConversation, handleUnmuteConversation,
      handleAcceptFileTransfer, handleRejectFileTransfer,
    }}>
      {children}
    </ChatContext.Provider>
  );
}
