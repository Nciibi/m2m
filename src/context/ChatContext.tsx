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
  messages: ChatMessage[];
  fileRequests: FileRequest[];
  conversations: ConversationEntry[];
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
  handleSendMessage: (content: string) => Promise<void>;
  handleVerify: () => Promise<void>;
  handleDisconnect: () => Promise<void>;
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
  handleSendMessageWithTimer: (content: string, disappearAfter?: number) => Promise<void>;
  handleEditMessage: (messageId: string, newContent: string) => Promise<void>;
  handleDeleteMessage: (messageId: string) => Promise<void>;
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
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [fileRequests, setFileRequests] = useState<FileRequest[]>([]);
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

  const handleSendMessage = useCallback(async (content: string) => {
    if (!connection?.peer_key_hex) return;
    const msg = await invoke<ChatMessage>("send_message", {
      peerKeyHex: connection.peer_key_hex,
      content,
    });
    setMessages((prev) => [...prev, msg]);
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
        reactions: {},
      }]);
    } catch (e) {
      addToast("Failed to send file: " + e, "error");
    }
  }, [connection?.peer_key_hex, addToast]);

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
  }, [setView]);

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

  const handleSendMessageWithTimer = useCallback(async (content: string, disappearAfter?: number) => {
    if (!connection?.peer_key_hex) return;
    const msg = await invoke<ChatMessage>("send_message_with_timer", {
      peerKeyHex: connection.peer_key_hex,
      content,
      disappearAfter: disappearAfter ?? null,
    });
    setMessages((prev) => [...prev, msg]);
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

  // ─── Tauri event listeners ───
  const [notifPermission, setNotifPermission] = useState(false);

  useEffect(() => {
    (async () => {
      const { isPermissionGranted, requestPermission } = await import("@tauri-apps/plugin-notification");
      let granted = await isPermissionGranted();
      if (!granted) { const result = await requestPermission(); granted = result === "granted"; }
      setNotifPermission(granted);
    })();
  }, []);

  useEffect(() => {
    const unlistenMsg = listen<any>("m2m://message", (event) => {
      setMessages((prev) => [...prev, event.payload.message]);
    });

    const unlistenConn = listen<any>("m2m://connection", async (event) => {
      const stateStr = event.payload.state;
      setConnection({
        state: stateStr,
        peer_fingerprint: event.payload.peer_fingerprint,
        peer_verified: false,
        peer_key_hex: event.payload.peer_key_hex,
      });
      if (stateStr === "established") {
        setActiveConversationId(event.payload.peer_key_hex);
        setView("chat");
        try {
          setMessages(await invoke<ChatMessage[]>("load_messages", { peerKeyHex: event.payload.peer_key_hex }));
        } catch { /* noop */ }
      } else if (stateStr === "disconnected") {
        setView("hub");
        setConnection(null);
        setMessages([]);
        setActiveConversationId(null);
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

    const unlistenDelete = listen<any>("m2m://delete", (event) => {
      const { message_id } = event.payload;
      setMessages((prev) => prev.map((m) =>
        m.id === message_id
          ? { ...m, deleted: true, content: "[deleted]" }
          : m
      ));
    });

    return () => {
      unlistenMsg.then((f) => f());
      unlistenConn.then((f) => f());
      unlistenFileReq.then((f) => f());
      unlistenFileComp.then((f) => f());
      unlistenConvMeta.then((f) => f());
      unlistenReaction.then((f) => f());
      unlistenEdit.then((f) => f());
      unlistenDelete.then((f) => f());
    };
  }, [setView, notifPermission]);

  return (
    <ChatContext.Provider value={{
      connection, isConnecting, messages, fileRequests,
      conversations, activeConversationId,
      inviteToConnect, setInviteToConnect, inviteValid,
      namingMyName, setNamingMyName, namingTheirName, setNamingTheirName,
      generatedInvite,
      retentionPolicy, setRetentionPolicy, retentionDuration, setRetentionDuration,
      handleSendMessage, handleVerify, handleDisconnect, handleSendFile,
      handleExportConversation, handleSetRetention,
      handleGenerateInvite, copyInvite, handleConnect, handleOpenChat,
      handleDeleteConversation,
      handleSendReaction, handleRemoveReaction, handleMarkConversationRead,
      handleSendMessageWithTimer, handleEditMessage, handleDeleteMessage,
    }}>
      {children}
    </ChatContext.Provider>
  );
}
