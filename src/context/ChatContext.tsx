import {
  createContext, useContext, useState, useEffect, useCallback, useMemo, useRef, ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  asConnectionEvent,
  asConversationMeta,
  asDeleteEvent,
  asEditEvent,
  asFileRequestEvent,
  asMessageEvent,
  asReactionEvent,
  asReconnectAttempt,
  asTransferCancelledEvent,
  asTransferCompletedEvent,
  asTransferErrorEvent,
  asTransferProgressEvent,
  asTypingEvent,
} from "../events";
import { listen } from "@tauri-apps/api/event";
import { useApp } from "./AppContext";
import { errorMessage } from "../utils";
import { asArray } from "../events";
import { useT } from "../i18n/I18nContext";
import type {
  ChatMessage,
  ConnectionInfo,
  ConversationEntry,
  FileRequest,
  InviteInfo,
  TransferProgress,
} from "../types";

/**
 * Coerce a backend result into a list.
 *
 * Every one of these values is rendered with `.length` and iterated, so a
 * null/undefined from the backend crashes the tree on the next render — and
 * `invoke` returning null is entirely possible (a locked vault, a command that
 * short-circuits, a deserialisation that yields nothing). Validate the shape
 * rather than trusting it.
 */
function asList<T>(v: unknown): T[] {
  // Delegates to the shared guard in `events.ts`. This was an independent
  // 3-line copy, which is how the same problem ended up solved twice and fixed
  // at 6 of ~50 call sites.
  return asArray<T>(v);
}

interface ChatContextValue {
  connection: ConnectionInfo | null;
  isConnecting: boolean;
  reconnecting: boolean;
  reconnectAttempt: number;
  messages: ChatMessage[];
  setMessages: React.Dispatch<React.SetStateAction<ChatMessage[]>>;
  fileRequests: FileRequest[];
  transfers: TransferProgress[];
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
  /** Send a file whose path is already known (drag-and-drop). */
  sendFileAtPath: (filePath: string) => Promise<void>;
  handleExportConversation: () => Promise<void>;
  handleSetRetention: (policy: string, durationSecs: number | null) => Promise<void>;
  handleGenerateInvite: () => Promise<void>;
  copyInvite: () => Promise<boolean>;
  handleConnect: () => Promise<void>;
  handleOpenChat: (conv: ConversationEntry) => Promise<void>;
  handleDeleteConversation: (conversationId: string) => Promise<void>;
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
  handleAcceptFileTransfer: (req: FileRequest) => Promise<void>;
  handleRejectFileTransfer: (req: FileRequest) => Promise<void>;
}

const ChatContext = createContext<ChatContextValue | null>(null);

export function useChat(): ChatContextValue {
  const ctx = useContext(ChatContext);
  if (!ctx) throw new Error("useChat() must be used within <ChatProvider>");
  return ctx;
}

export function ChatProvider({ children }: { children: ReactNode }) {
  const { addToast, setView, identity } = useApp();
  const t = useT();

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
      setConversations(asList<ConversationEntry>(await invoke("list_conversations")));
    } catch { /* noop */ }
  }, []);

  // ─── Handlers ───

  // The peer key as a primitive, for handlers to close over.
  //
  // These callbacks previously listed `connection?.peer_key_hex` as their
  // dependency while reading `connection.peer_key_hex` in the body. React's
  // compiler infers `connection` (the whole object) from the body, sees a
  // mismatch, and then SKIPS COMPILING THE ENTIRE COMPONENT — so all ~30
  // useCallbacks in this file were left unoptimised, silently defeating the
  // "biggest render-cost fix in the app" the comment below claims.
  //
  // Reading a primitive that IS the declared dependency lets the compiler
  // preserve every memo here, and makes these handlers stable across
  // `connection` object identity changes (a fresh ConnectionInfo with the same
  // key no longer invalidates them).
  const peerKeyHex = connection?.peer_key_hex;

  // Mirror of `activeConversationId` for the long-lived listeners, plus a single
  // setter that writes *both* synchronously.
  //
  // Syncing the ref only in an effect leaves it one commit stale, and the
  // `m2m://connection` handler reads it to decide whether an inbound
  // `established` event may take over the view (see `adoptingPeer` there). In
  // that one-commit window the ref still held the previous value — so a peer
  // completing a handshake immediately after the user opened a different
  // conversation could still hijack it, which is the misdelivery the guard
  // exists to prevent.
  const activeConversationIdRef = useRef(activeConversationId);
  const setActiveConversation = useCallback((peerKeyHex: string | null) => {
    activeConversationIdRef.current = peerKeyHex;
    setActiveConversationId(peerKeyHex);
  }, []);

  const handleSendMessage = useCallback(async (content: string): Promise<ChatMessage> => {
    if (!peerKeyHex) throw new Error("Not connected");
    const msg = await invoke<ChatMessage>("send_message", {
      peerKeyHex: peerKeyHex,
      content,
    });
    setMessages((prev) => [...prev, msg]);
    return msg;
  }, [peerKeyHex]);

  const handleVerify = useCallback(async () => {
    if (!connection?.peer_key_hex) {
      throw new Error("No active peer to verify");
    }
    // Let failures propagate.
    //
    // This used to swallow the error, and the caller in ChatView does
    // `await handleVerify(); addToast("Peer verified", "success")` — so a
    // failed verification still showed a green "Peer verified" confirmation.
    // That is a false trust confirmation on the one interaction that anchors
    // the user's trust model, and the single worst place in the app to lie.
    await invoke("verify_peer", { peerKeyHex: connection.peer_key_hex });
    setConnection({ ...connection, peer_verified: true });
  }, [connection]);

  const handleDisconnect = useCallback(async () => {
    if (!peerKeyHex) return;
    try {
      await invoke("disconnect_peer", { peerKeyHex: peerKeyHex });
      setView("hub");
      setConnection(null);
      setMessages([]);
    } catch { /* noop */ }
  }, [peerKeyHex, setView]);

  /// Send one file at a known path and show the optimistic local row.
  ///
  /// Split out of `handleSendFile` so drag-and-drop can use it. The drop
  /// handler used to toast `"Dropped <name> — sending..."` and then call the
  /// picker-based handler, so the dropped file was never sent and the toast
  /// named a file that was not being transmitted.
  const sendFileAtPath = useCallback(async (filePath: string) => {
    if (!peerKeyHex) throw new Error("Not connected");
    await invoke("send_file", { peerKeyHex: peerKeyHex, filePath });
    const filename = filePath.split(/[\\/]/).pop() || "file";
    // An optimistic local row so the send feels immediate. Built as a
    // `ChatMessage` rather than cast into one: the previous `as ChatMessage`
    // suppressed the compiler on exactly the fields that reach a className
    // and a text node, including a peer-influenced `filename`.
    //
    // `crypto.randomUUID()` rather than `Date.now().toString()`: the id is
    // used as a React key and as the handle later edits and reactions
    // address, and two sends in the same millisecond produced a duplicate key.
    //
    // `sender_peer_key_hex` is left empty, matching `ChatMessage::new` on the
    // Rust side: it names the *sender*, and for a 1:1 message that is implicit
    // from the conversation. Setting it to the remote peer's key made
    // `MessageBubble` render the peer's key prefix as the label on the user's
    // own outgoing bubble.
    const optimistic: ChatMessage = {
      id: crypto.randomUUID(),
      content: `File request sent: ${filename}`,
      direction: "sent",
      timestamp: Math.floor(Date.now() / 1000),
      read_at: null,
      edited_at: null,
      deleted: false,
      expires_at: null,
      reactions: {},
      sender_peer_key_hex: "",
    };
    setMessages((prev) => [...prev, optimistic]);
  }, [peerKeyHex]);

  const handleSendFile = useCallback(async () => {
    if (!peerKeyHex) return;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({ multiple: false, title: "Select file to send" });
      if (!selected) return;
      const filePath = typeof selected === "string" ? selected : selected;
      await sendFileAtPath(filePath);
    } catch (e) {
      addToast("Failed to send file: " + errorMessage(e), "error");
    }
  }, [peerKeyHex, addToast, sendFileAtPath]);

  const handleAcceptFileTransfer = useCallback(async (req: FileRequest) => {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const savePath = await save({ title: `Save "${req.filename}"`, defaultPath: req.filename });
      if (!savePath) return;
      await invoke("accept_file_transfer", {
        peerKeyHex: req.peer_key_hex,
        transferId: req.transfer_id,
        saveDir: savePath,
      });
      setFileRequests((prev) => prev.filter((r) => r.transfer_id !== req.transfer_id));
      addToast("Downloading file...", "info");
    } catch (e) {
      addToast("Failed to accept transfer: " + errorMessage(e), "error");
    }
  }, [addToast]);

  const handleRejectFileTransfer = useCallback(async (req: FileRequest) => {
    try {
      await invoke("reject_file_transfer", {
        peerKeyHex: req.peer_key_hex,
        transferId: req.transfer_id,
      });
      setFileRequests((prev) => prev.filter((r) => r.transfer_id !== req.transfer_id));
      addToast("File transfer rejected", "info");
    } catch (e) {
      addToast("Failed to reject transfer: " + errorMessage(e), "error");
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
      addToast("Export failed: " + errorMessage(e), "error");
    }
  }, [activeConversationId, addToast]);

  const handleSetRetention = useCallback(async (policy: string, durationSecs: number | null) => {
    if (!activeConversationId) return;
    const conversationId = activeConversationId;
    const previousPolicy = retentionPolicy;
    const previousDuration = retentionDuration;
    try {
      await invoke("set_conversation_retention", { conversationId, policy, durationSecs });
    } catch (e) {
      // Retention is a data-lifecycle control, and this was a silent no-op on
      // failure. The <select> in ChatView has already moved to the new value
      // by the time this runs, so a user who selects "Auto-Delete After 24
      // Hours" on a failed write is shown a policy that will not delete
      // anything — and the messages stay on disk, indefinitely, while the UI
      // says otherwise. Roll the control back and say what happened.
      setRetentionPolicy(previousPolicy);
      setRetentionDuration(previousDuration);
      addToast("Retention policy not saved: " + errorMessage(e), "error");
    }
  }, [activeConversationId, retentionPolicy, retentionDuration, addToast]);

  // Returns whether an invite was actually created.
  //
  // This was `async () => { try {...} catch { toast } }` — it reported success
  // by returning `undefined`, and the caller could not distinguish a real invite
  // from a failure. It toasts the failure itself, so the caller's `finally` ran
  // on both paths, which meant `HubView` set a countdown and "Listening for
  // incoming connections" for an invite that had never been created. A
  // one-time invite that never left the app looked exactly like one that had.
  const handleGenerateInvite = useCallback(async (): Promise<boolean> => {
    try {
      await invoke("start_listening", { address: "0.0.0.0:0" });
      const address = await invoke<string>("get_listen_address");
      const invite = await invoke<string>("create_invite", { address, validityMinutes: 60, oneTime: true });
      setGeneratedInvite(invite);
      return true;
    } catch (e) {
      addToast(errorMessage(e), "error", 6000);
      return false;
    }
  }, [addToast]);

  const copyInvite = useCallback(async () => {
    // Was fire-and-forget: `writeText` rejects if the window is unfocused or
    // permission is denied, and the caller unconditionally showed a green ✓
    // afterwards — so a one-time invite that never reached the clipboard was
    // reported as shared. In a Tauri webview this rejection is routine.
    try {
      await navigator.clipboard.writeText(generatedInvite);
      return true;
    } catch (e) {
      addToast("Could not copy to the clipboard: " + errorMessage(e, "clipboard unavailable"), "error");
      return false;
    }
  }, [generatedInvite, addToast]);

  const handleConnect = useCallback(async () => {
    if (!inviteToConnect) return;
    setIsConnecting(true);
    try {
      const info = await invoke<ConnectionInfo>("connect_to_peer", { inviteStr: inviteToConnect });
      setConnection(info);
      setActiveConversation(info.peer_key_hex || null);
      if (info.peer_key_hex && (namingMyName || namingTheirName)) {
        await invoke("send_conversation_names", {
          peerKeyHex: info.peer_key_hex, myName: namingMyName, theirName: namingTheirName,
        }).catch(() => {});
      }
      setView("chat");
      try {
        setMessages(asList<ChatMessage>(await invoke("load_messages", { peerKeyHex: info.peer_key_hex })));
      } catch { /* noop */ }
    } catch (e) {
      addToast("Connection failed: " + errorMessage(e), "error");
    } finally {
      setIsConnecting(false);
    }
  }, [inviteToConnect, namingMyName, namingTheirName, addToast, setView, setActiveConversation]);

  const handleOpenChat = useCallback(async (conv: ConversationEntry) => {
    setActiveConversation(conv.peer_key_hex);
    setRetentionPolicy(conv.retention_policy || "none");
    setView("chat");
    // The backend is the only thing that knows whether this peer was ever
    // fingerprint-verified, and it is not persisted in the conversation list —
    // so ask it. This used to hard-code `peer_verified: true`, which put a
    // green "Verified" badge on every conversation opened from the Hub (the
    // most common way in), and, because the fingerprint modal hides its
    // "Confirm Match & Verify" button once `peer_verified` is set, removed the
    // user's only way to actually verify. Opening a conversation is not a
    // verification event.
    setConnection({
      state: conv.is_online ? "established" : "disconnected",
      peer_fingerprint: null,
      peer_verified: false,
      peer_key_hex: conv.peer_key_hex,
    });
    try {
      const live = await invoke<ConnectionInfo>("get_connection_state", {
        peerKeyHex: conv.peer_key_hex,
      });
      if (live) {
        setConnection({
          state: live.state ?? "disconnected",
          peer_fingerprint: live.peer_fingerprint ?? null,
          peer_verified: live.peer_verified === true,
          peer_key_hex: conv.peer_key_hex,
        });
      }
    } catch {
      // No live session, or the command failed. The disconnected/unverified
      // state set above is the honest fallback: a peer with no session has not
      // been verified in this session.
    }
    try {
      setMessages(asList<ChatMessage>(await invoke("load_messages", { peerKeyHex: conv.peer_key_hex })));
    } catch { /* noop */ }
    // Mark messages as read when opening a conversation
    try {
      await invoke("mark_messages_read", { conversationId: conv.peer_key_hex });
    } catch { /* noop */ }
    // Refresh conversation list to update unread counts
    loadConversations();
  }, [setView, loadConversations, setActiveConversation]);

  // Owns the delete, including the IPC call.
  //
  // This was `() => void` that only reloaded the list, while the actual
  // `invoke("delete_conversation_cmd")` was inlined at the call site in
  // HubView. `ChatsTab` typed the prop as `(conversationId: string) => void`,
  // but a zero-parameter function is assignable to that, so TypeScript stayed
  // silent while the id was discarded — the exact bug the HubView comment
  // claims was fixed by adding the annotation. The annotation silenced the
  // compiler; it did not fix the split.
  //
  // Doing the delete here means the id is genuinely consumed, and the toast
  // goes through the same path as every other command error.
  const handleDeleteConversation = useCallback(async (conversationId: string) => {
    try {
      await invoke("delete_conversation_cmd", { conversationId });
      await loadConversations();
    } catch (e) {
      addToast("Failed to delete conversation: " + errorMessage(e), "error");
    }
  }, [loadConversations, addToast]);

  // ─── Reaction handlers ───
  //
  // The optimistic write uses our **real** Ed25519 key, not a `"self"`
  // sentinel. The backend persists the key (`send_reaction` →
  // `upsert_reaction(..., &peer_key_hex, ...)`), so a sentinel diverged from
  // the stored shape the instant the page was reloaded: the chip went
  // unhighlighted and a second click re-sent the reaction instead of removing
  // it. It also violated `events.ts`, which validates every reactor with
  // `isPeerKeyHex` — so `"self"` was a value this app's own event boundary
  // would reject. `MessageBubble` compares against the same key.
  const myPeerKey = identity?.public_key_hex ?? "";

  const handleSendReaction = useCallback(async (messageId: string, reaction: string) => {
    if (!peerKeyHex) return;
    try {
      await invoke("send_reaction", { peerKeyHex: peerKeyHex, messageId, reaction });
      // Optimistically update UI
      setMessages((prev) => prev.map((m) => {
        if (m.id !== messageId) return m;
        const reactions = { ...m.reactions };
        const reactors = reactions[reaction] || [];
        if (myPeerKey && !reactors.includes(myPeerKey)) {
          reactions[reaction] = [...reactors, myPeerKey];
        }
        return { ...m, reactions };
      }));
    } catch (e) {
      // The optimistic update above was the whole point, but it must be undone
      // on failure — otherwise the user sees a reaction that the peer will
      // never receive, and believes it was delivered.
      addToast("Reaction failed: " + errorMessage(e), "error");
      // Delete the key when nothing is left, exactly as `handleRemoveReaction`
      // and the `m2m://reaction` listener do. Leaving `{"👍": []}` behind makes
      // `MessageBubble` render a chip reading "0" for a reaction that never
      // reached the peer — the same phantom-state problem in a new place.
      setMessages((prev) => prev.map((m) => {
        if (m.id !== messageId) return m;
        const reactions = { ...m.reactions };
        const remaining = (reactions[reaction] || []).filter((r) => r !== myPeerKey);
        if (remaining.length === 0) {
          delete reactions[reaction];
        } else {
          reactions[reaction] = remaining;
        }
        return { ...m, reactions };
      }));
    }
  }, [peerKeyHex, addToast, myPeerKey]);

  const handleRemoveReaction = useCallback(async (messageId: string, reaction: string) => {
    if (!peerKeyHex) return;
    try {
      await invoke("remove_reaction", { peerKeyHex: peerKeyHex, messageId, reaction });
      // Optimistically update UI
      setMessages((prev) => prev.map((m) => {
        if (m.id !== messageId) return m;
        const reactions = { ...m.reactions };
        const reactors = (reactions[reaction] || []).filter((r: string) => r !== myPeerKey);
        if (reactors.length === 0) {
          delete reactions[reaction];
        } else {
          reactions[reaction] = reactors;
        }
        return { ...m, reactions };
      }));
    } catch (e) {
      addToast("Could not remove reaction: " + errorMessage(e), "error");
      // Roll back by re-adding our key — the state before the failed removal.
      setMessages((prev) => prev.map((m) =>
        m.id === messageId && myPeerKey
          ? { ...m, reactions: { ...m.reactions, [reaction]: [...new Set([...(m.reactions[reaction] ?? []), myPeerKey])] } }
          : m,
      ));
    }
  }, [peerKeyHex, addToast, myPeerKey]);

  const handleReconnect = useCallback(async () => {
    if (!connection?.peer_key_hex) return;
    setReconnecting(true);
    setReconnectAttempt(1);
    try {
      const info = await invoke<ConnectionInfo>("attempt_reconnect", { peerKeyHex: connection.peer_key_hex });
      setConnection(info);
    } catch {
      // Failure to reconnect is not actionable here; the caller retries on
      // the next backoff tick. The old code bound `e` and never read it.
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
    if (!peerKeyHex) throw new Error("Not connected");
    const msg = await invoke<ChatMessage>("send_message_with_timer", {
      peerKeyHex: peerKeyHex,
      content,
      disappearAfter: disappearAfter ?? null,
    });
    setMessages((prev) => [...prev, msg]);
    return msg;
  }, [peerKeyHex]);

  const handleEditMessage = useCallback(async (messageId: string, newContent: string) => {
    if (!peerKeyHex) return;
    try {
      const updated = await invoke<ChatMessage>("edit_message", {
        peerKeyHex: peerKeyHex,
        messageId,
        newContent,
      });
      setMessages((prev) => prev.map((m) => m.id === messageId ? updated : m));
    } catch (e) {
      addToast("Edit failed: " + errorMessage(e), "error");
      // Rethrow. `MessageBubble` awaited this and closed its editor
      // unconditionally, so a failed save discarded the user's retyped text and
      // looked identical to a successful edit. The bubble now keeps the editor
      // open on rejection; the toast above is already the user-facing report.
      throw e;
    }
  }, [peerKeyHex, addToast]);

  const handleDeleteMessage = useCallback(async (messageId: string) => {
    if (!peerKeyHex) return;
    try {
      await invoke("delete_message", {
        peerKeyHex: peerKeyHex,
        messageId,
      });
      // Optimistic update — mark as deleted immediately
      setMessages((prev) => prev.map((m) =>
        m.id === messageId ? { ...m, deleted: true, content: "[deleted]" } : m
      ));
    } catch (e) {
      addToast("Delete failed: " + errorMessage(e), "error");
    }
  }, [peerKeyHex, addToast]);

  // ─── Invite validation effect ───
  useEffect(() => {
    if (inviteToConnect.length > 30) {
      invoke<InviteInfo>("validate_invite", { inviteStr: inviteToConnect })
        .then((info) => { if (info?.valid) setInviteValid(true); })
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
    try {
      const muted = await invoke<string[] | null>("get_muted_conversations");
      // Feeds `mutedConversations.includes(...)` and is rendered, so a
      // non-array result would crash the tree. Validate rather than trust.
      setMutedConversations(Array.isArray(muted) ? muted : []);
    } catch { /* noop */ }
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
    // Was a silent no-op on failure. Mute is a safety control — the user
    // believes a conversation is suppressed when it is not, and learns that
    // from the message they were counting on not seeing.
    try {
      await invoke("mute_conversation", { peerKeyHex });
      await loadMutedConversations();
    } catch (e) {
      addToast("Could not mute: " + errorMessage(e), "error");
    }
  }, [loadMutedConversations, addToast]);

  const handleUnmuteConversation = useCallback(async (peerKeyHex: string) => {
    try {
      await invoke("unmute_conversation", { peerKeyHex });
      await loadMutedConversations();
    } catch (e) {
      addToast("Could not unmute: " + errorMessage(e), "error");
    }
  }, [loadMutedConversations, addToast]);

  // ─── Tauri event listeners ───
  //
  // The three values the handlers read but must NOT depend on are mirrored into
  // refs. The effect used to list `activeConversationId`, `mutedConversations`
  // and `notifPermission` as dependencies, so every time the user opened or
  // switched conversation all 13 listeners were torn down and re-registered —
  // 13 unlisten round-trips plus 13 registrations across the IPC bridge.
  //
  // Worse, `listen()` is async. Between the teardown and the re-registration
  // completing there was NO `m2m://message` listener registered, and messages
  // arriving in that window were silently dropped. Opening a conversation
  // could therefore lose a message. Reading through refs removes the churn
  // entirely and closes the window.
  const notifPermissionRef = useRef(notifPermission);
  // The 13 event listeners are registered once and must stay registered, so they
  // cannot close over `t` directly: it is rebuilt on every locale change, which
  // would tear down and re-register the whole bridge. Reading it through a ref
  // gives the listeners live translations without any re-registration.
  const tRef = useRef(t);
  useEffect(() => { tRef.current = t; }, [t]);
  const mutedConversationsRef = useRef(mutedConversations);
  useEffect(() => { notifPermissionRef.current = notifPermission; }, [notifPermission]);
  useEffect(() => { activeConversationIdRef.current = activeConversationId; }, [activeConversationId]);
  useEffect(() => { mutedConversationsRef.current = mutedConversations; }, [mutedConversations]);

  // Navigation intents queued by the notification handler, drained by a
  // separate effect. See the comment at `drainNavigationIntent` for why the
  // listener cannot navigate directly.
  const navIntentRef = useRef<{ peerKeyHex: string } | null>(null);
  const [, forceNavRender] = useState(0);

  useEffect(() => {
    if (navIntentRef.current) {
      const { peerKeyHex } = navIntentRef.current;
      navIntentRef.current = null;
      setActiveConversation(peerKeyHex);
      setView("chat");
      invoke("load_messages", { peerKeyHex })
        .then((r) => setMessages(asList<ChatMessage>(r)))
        .catch((e) => addToast("Could not open conversation: " + errorMessage(e), "error"));
    }
  }, [setActiveConversation, setView, addToast]);

  useEffect(() => {
    const unlistenMsg = listen("m2m://message", (event) => {
      // Validate before touching state. The payload is peer-controlled: the
      // body goes to `renderMarkdown`, `peer_key_hex` becomes a SQLite lookup
      // key and a notification group.
      const payload = asMessageEvent(event.payload);
      if (!payload) {
        console.warn("M2M: dropping malformed m2m://message payload");
        return;
      }
      const message = payload.message;
      const peerKeyHex = payload.peer_key_hex;

      // Only append to the transcript if this message belongs to the
      // conversation actually on screen.
      //
      // Without this, a message from any peer was appended to whatever
      // conversation was open: reading source B while informant C writes, and
      // C's message appears in B's transcript — with C's sender label but
      // inside B's session banner, and any reply goes to B. Reactions, edits
      // and deletes then resolve by `message_id` against a message from a
      // different conversation. The peer key was read two lines below, but
      // only to decide whether to fire an OS notification.
      const forActiveConversation = peerKeyHex === activeConversationIdRef.current;
      if (forActiveConversation) {
        setMessages((prev) => [...prev, message]);
      } else {
        // The conversation list still has to learn about it, or the Hub shows
        // no unread indicator and the user never learns a message arrived.
        setConversations((prev) =>
          prev.map((c) =>
            c.peer_key_hex === peerKeyHex
              ? { ...c, last_message_preview: message.content, last_message_at: message.timestamp }
              : c,
          ),
        );
      }

      // Send native OS notification if:
      // 1. Notification permission granted
      // 2. Not currently viewing this conversation
      // 3. Conversation is not muted
      if (
        notifPermissionRef.current
        && peerKeyHex !== activeConversationIdRef.current
        && !mutedConversationsRef.current.includes(peerKeyHex)
      ) {
        // The Rust `MessageEvent` carries no fingerprint — the old code read
        // one from two places, got `undefined` from both, and fell through to
        // the peer-key prefix anyway. So use the key prefix directly rather
        // than keeping a variable that is provably always null.
        const displayName = peerKeyHex.substring(0, 8) + "…";
        import("@tauri-apps/plugin-notification").then(({ sendNotification, isPermissionGranted: _i }) => {
          sendNotification({
            title: "M2M",
            body: `New message from ${displayName}`,
            group: peerKeyHex,
          });
        });
        // Record an intent to open this conversation, then nudge the app to
        // return to the foreground so the user sees it.
        //
        // The previous code called `document.addEventListener(
        // "visibilitychange", …, { once: true })` here. That had two problems:
        // it leaked one document-level listener per unread message (the effect
        // cleanup never removed them, and `once: true` only fires on an actual
        // visibility change), and it hijacked the next window-visibility event
        // to force-navigate to the last peer's conversation, discarding
        // whatever the user was doing. A queued intent is explicit, has no
        // listener to leak, and cannot fire spuriously.
        navIntentRef.current = { peerKeyHex };
        try { window.focus(); } catch { /* not all platforms allow this */ }
        forceNavRender((n) => n + 1);
      }
    });

    const unlistenConn = listen("m2m://connection", async (event) => {
      // `state` drives view navigation and whether a Reconnect button is
      // shown, so an unrecognised value must not be acted on.
      const conn = asConnectionEvent(event.payload);
      if (!conn) {
        console.warn("M2M: dropping malformed m2m://connection payload");
        return;
      }
      const stateStr = conn.state;
      // A connection event is a statement about ONE peer, not about the session
      // the user is currently looking at.
      //
      // `setActiveConversationId(conn.peer_key_hex)` used to run unconditionally
      // for every `established` event, so any peer completing a handshake — a
      // known contact, a family member, or with `require_known_contact` off any
      // stranger holding an invite — silently switched the open conversation to
      // itself and navigated to the chat view.
      //
      // The composer text is local state in ChatView and is never cleared on a
      // conversation change, and `peerKeyHex` (which every send handler closes
      // over) followed the switch. So a message the user was composing for peer
      // B, with peer B still on screen and in the header, was delivered to peer
      // C. The `m2m://message` listener below was already hardened against
      // cross-conversation contamination; this was the same hole one level up.
      //
      // Adopt the connection only when no peer is already open, or when it is
      // this same peer. Otherwise update the conversation list (below) and
      // leave the view alone.
      const openPeer = activeConversationIdRef.current;
      const adoptingPeer = openPeer === null || openPeer === conn.peer_key_hex;

      if (adoptingPeer) {
        setConnection({
          state: stateStr,
          peer_fingerprint: conn.peer_fingerprint,
          peer_verified: conn.peer_verified,
          peer_key_hex: conn.peer_key_hex,
        });
      }
      if (stateStr === "established") {
        if (!adoptingPeer) {
          // Not ours to adopt — but the conversation list should still reflect
          // that this peer is now online.
          try {
            setConversations(asList<ConversationEntry>(await invoke("list_conversations")));
          } catch { /* noop */ }
          return;
        }
        setReconnecting(false);
        setReconnectAttempt(0);
        // `setActiveConversation` writes both the state and the ref, synchronously.
        setActiveConversation(conn.peer_key_hex);
        setView("chat");
        try {
          setMessages(asList<ChatMessage>(await invoke("load_messages", { peerKeyHex: conn.peer_key_hex })));
        } catch { /* noop */ }
      } else if (stateStr === "disconnected") {
        // Only meaningful for the conversation actually on screen — but a peer
        // going offline is always worth refreshing the list for, so this must not
        // `return` past the refresh below. (It used to, which left a peer shown
        // as online in the Hub until some unrelated event refreshed it.)
        if (adoptingPeer && !conn.peer_verified) {
          // For verified peers, stay on ChatView so the user can attempt a
          // reconnect. For unverified peers, go back to the hub — there is no
          // reconnect to offer.
          setView("hub");
          setConnection(null);
          setMessages([]);
          setActiveConversation(null);
        }
      }
      try { setConversations(asList<ConversationEntry>(await invoke("list_conversations"))); } catch { /* noop */ }
    });

    const unlistenConvMeta = listen("m2m://conversation-meta", async (event) => {
      // The suggested name is peer-supplied and the backend writes it into the
      // conversation table, so it is validated before we act on the event.
      const meta = asConversationMeta(event.payload);
      if (!meta) {
        console.warn("M2M: dropping malformed m2m://conversation-meta payload");
        return;
      }
      try { setConversations(asList<ConversationEntry>(await invoke("list_conversations"))); } catch { /* noop */ }
    });

    const unlistenFileReq = listen("m2m://file-request", (event) => {
      // `filename` is peer-controlled and is rendered, used in a dialog title
      // and passed to a save-path default.
      const req = asFileRequestEvent(event.payload);
      if (!req) {
        console.warn("M2M: dropping malformed m2m://file-request payload");
        return;
      }
      setFileRequests((prev) => [...prev, req]);
    });

    const unlistenFileProgress = listen("m2m://transfer-progress", (event) => {
      // `state` is interpolated into a className downstream, so it is
      // constrained to the backend's known set.
      const progress = asTransferProgressEvent(event.payload);
      if (!progress) {
        console.warn("M2M: dropping malformed m2m://transfer-progress payload");
        return;
      }
      setTransfers((prev: TransferProgress[]) => {
        const idx = prev.findIndex((t) => t.transfer_id === progress.transfer_id);
        if (idx >= 0) {
          const updated = [...prev];
          updated[idx] = progress;
          return updated;
        }
        return [...prev, progress];
      });
    });

    const unlistenFileCompleted = listen("m2m://transfer-completed", (event) => {
      const done = asTransferCompletedEvent(event.payload);
      if (!done) {
        console.warn("M2M: dropping malformed m2m://transfer-completed payload");
        return;
      }
      setTransfers((prev) => prev.filter((t) => t.transfer_id !== done.transfer_id));
      // The backend sends ONLY `transfer_id` on this event — the old code read
      // a `filename` that is never present, so the toast always rendered with
      // an empty suffix.
      addToast(tRef.current("toast.transferComplete", { filename: "" }), "success");
    });

    const unlistenFileError = listen("m2m://transfer-error", (event) => {
      const failed = asTransferErrorEvent(event.payload);
      if (!failed) {
        console.warn("M2M: dropping malformed m2m://transfer-error payload");
        return;
      }
      setTransfers((prev) => prev.filter((t) => t.transfer_id !== failed.transfer_id));
      addToast(
        tRef.current("toast.transferFailed", {
          err: failed.error || tRef.current("toast.transferFailedUnknown"),
        }),
        "error",
      );
    });

    const unlistenFileCancelled = listen("m2m://transfer-cancelled", (event) => {
      const cancelled = asTransferCancelledEvent(event.payload);
      if (!cancelled) {
        console.warn("M2M: dropping malformed m2m://transfer-cancelled payload");
        return;
      }
      setTransfers((prev) => prev.filter((t) => t.transfer_id !== cancelled.transfer_id));
      addToast(tRef.current("toast.transferCancelled"), "warning");
    });

    const unlistenReaction = listen("m2m://reaction", (event) => {
      // `reaction` becomes an object key AND a visible label; unbounded
      // peer input there is both a rendering and a memory hazard.
      const rxn = asReactionEvent(event.payload);
      if (!rxn) {
        console.warn("M2M: dropping malformed m2m://reaction payload");
        return;
      }
      const { message_id, reaction, peer_key_hex, remove } = rxn;
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

    const unlistenEdit = listen("m2m://edit", (event) => {
      // `new_content` replaces the rendered body and reaches renderMarkdown.
      const edit = asEditEvent(event.payload);
      if (!edit) {
        console.warn("M2M: dropping malformed m2m://edit payload");
        return;
      }
      const { message_id, new_content, edited_at } = edit;
      setMessages((prev) => prev.map((m) =>
        m.id === message_id
          ? { ...m, content: new_content, edited_at }
          : m
      ));
    });

    const unlistenReconnectAttempt = listen("m2m://reconnect-attempt", (event) => {
      const attempt_ = asReconnectAttempt(event.payload);
      if (!attempt_) {
        console.warn("M2M: dropping malformed m2m://reconnect-attempt payload");
        return;
      }
      const { state: reconnectState, attempt } = attempt_;
      if (reconnectState === "attempting") {
        setReconnecting(true);
        setReconnectAttempt(attempt);
        return;
      }
      // "success", "failed" AND "handshake_failed" all end the attempt.
      //
      // The backend emits `handshake_failed` but the old handler only knew
      // about three states, so a handshake failure left `reconnecting` stuck
      // `true` and the UI showed "Reconnecting (N/5)…" forever.
      setReconnecting(false);
      setReconnectAttempt(0);
    });

    const unlistenDelete = listen("m2m://delete", (event) => {
      const del = asDeleteEvent(event.payload);
      if (!del) {
        console.warn("M2M: dropping malformed m2m://delete payload");
        return;
      }
      const { message_id } = del;
      setMessages((prev) => prev.map((m) =>
        m.id === message_id
          ? { ...m, deleted: true, content: "[deleted]" }
          : m
      ));
    });

    const unlistenTyping = listen("m2m://typing", (event) => {
      const typing_ = asTypingEvent(event.payload);
      if (!typing_) {
        console.warn("M2M: dropping malformed m2m://typing payload");
        return;
      }
      const { peer_key_hex: typingPeer, typing } = typing_;
      if (typing) {
        setTypingPeers((prev: string[]) => prev.includes(typingPeer) ? prev : [...prev, typingPeer]);
      } else {
        setTypingPeers((prev: string[]) => prev.filter((p: string) => p !== typingPeer));
      }
    });

    return () => {
      // `.catch()` on every teardown: an unhandled rejection here would surface
      // as an uncaught promise rejection during unmount, and one failing
      // unlisten must not prevent the other twelve from running.
      const stop = (p: Promise<() => void>) => { p.then((f) => f()).catch(() => {}); };
      stop(unlistenMsg);
      stop(unlistenConn);
      stop(unlistenFileReq);
      stop(unlistenFileProgress);
      stop(unlistenFileCompleted);
      stop(unlistenFileError);
      stop(unlistenFileCancelled);
      stop(unlistenConvMeta);
      stop(unlistenReaction);
      stop(unlistenEdit);
      stop(unlistenReconnectAttempt);
      stop(unlistenDelete);
      stop(unlistenTyping);
    };
    // Only genuinely stable dependencies. Adding any state value here tears
    // down and re-registers all 13 listeners, which both churns the IPC bridge
    // and opens a window where incoming messages are dropped. `addToast` and
    // `setView` are stable `useCallback`s; `setActiveConversation` is too (empty
    // deps), so listing it here does *not* re-register anything — and omitting
    // it would be a stale closure over a function that writes the ref the
    // connection listener reads.
  }, [setView, addToast, setActiveConversation]);

  /**
   * Memoized — this is the single biggest render-cost fix in the app.
   *
   * The ~30 handlers were all `useCallback`'d, but the wrapping object literal
   * was rebuilt on every render, which discards the benefit of all of them.
   * Combined with `useChat()` returning the whole context to `ChatView` and
   * `MessageBubble` not being `React.memo`'d, the effect was: typing one
   * character in the composer calls `handleTextChange` → `setText` → re-render
   * → a new context object → every message bubble in the transcript re-renders,
   * each re-running `renderMarkdown`. In a 500-message conversation that is
   * 500 component re-renders per keystroke.
   */
  const value = useMemo<ChatContextValue>(() => ({
    connection, isConnecting, reconnecting, reconnectAttempt, messages, setMessages, fileRequests, transfers,
    conversations, activeConversationId, typingPeers,
    inviteToConnect, setInviteToConnect, inviteValid,
    namingMyName, setNamingMyName, namingTheirName, setNamingTheirName,
    generatedInvite,
    retentionPolicy, setRetentionPolicy, retentionDuration, setRetentionDuration,
    handleSendMessage, handleVerify, handleDisconnect, handleReconnect, handleSendFile,
    sendFileAtPath,
    handleExportConversation, handleSetRetention,
    handleGenerateInvite, copyInvite, handleConnect, handleOpenChat,
    handleDeleteConversation,
    handleSendReaction, handleRemoveReaction, handleMarkConversationRead,
    handleSendMessageWithTimer, handleEditMessage, handleDeleteMessage,
    mutedConversations, handleMuteConversation, handleUnmuteConversation,
    handleAcceptFileTransfer, handleRejectFileTransfer,
  }), [
    connection, isConnecting, reconnecting, reconnectAttempt, messages, fileRequests, transfers,
    conversations, activeConversationId, typingPeers,
    inviteToConnect, inviteValid,
    namingMyName, namingTheirName, generatedInvite,
    retentionPolicy, retentionDuration,
    setMessages, setInviteToConnect, setNamingMyName, setNamingTheirName,
    setRetentionPolicy, setRetentionDuration,
    handleSendMessage, handleVerify, handleDisconnect, handleReconnect, handleSendFile,
    sendFileAtPath,
    handleExportConversation, handleSetRetention,
    handleGenerateInvite, copyInvite, handleConnect, handleOpenChat,
    handleDeleteConversation,
    handleSendReaction, handleRemoveReaction, handleMarkConversationRead,
    handleSendMessageWithTimer, handleEditMessage, handleDeleteMessage,
    mutedConversations, handleMuteConversation, handleUnmuteConversation,
    handleAcceptFileTransfer, handleRejectFileTransfer,
  ]);

  // ─── Scrub decrypted state when the vault locks ───
  //
  // `lock_vault` zeroizes the Rust keys, closes every store, drops the storage
  // key and emits `m2m://vault-locked`. `AppContext` handles that event, but
  // everything decrypted lives *here*, in a provider that sits above the view
  // switch and is therefore never unmounted by navigating to the unlock screen.
  //
  // The result was: after idle-lock or "Lock Now", every decrypted message body,
  // the peer fingerprint, pending file requests and a still-valid one-time invite
  // (with the user's own address embedded in it) stayed resident in JS heap for
  // the life of the process and reappeared the instant `messages` was next
  // rendered. The Rust half of this fix existed; the React half did not.
  //
  // `GroupChatView` holds its messages in component state and *is* unmounted by
  // the view switch, so only this long-lived provider leaks.
  useEffect(() => {
    let disposed = false;
    const stop = listen("m2m://vault-locked", () => {
      if (disposed) return;
      setMessages([]);
      setConnection(null);
      setFileRequests([]);
      setTransfers([]);
      setConversations([]);
      setTypingPeers([]);
      setActiveConversation(null);
      setGeneratedInvite("");
      setInviteToConnect("");
      setInviteValid(false);
      setIsConnecting(false);
      setReconnecting(false);
      setReconnectAttempt(0);
      // Also drop the per-conversation naming/retention/mute state: it is
      // identifying metadata and describes a conversation the vault can no
      // longer decrypt.
      setRetentionPolicy("none");
      setRetentionDuration("86400");
      setNamingMyName("");
      setNamingTheirName("");
      navIntentRef.current = null;
      // Must return an UnlistenFn from the rejection handler, not nothing:
      // `.catch(() => {})` types `stop` as `Promise<UnlistenFn | void>`, so the
      // teardown below calls a `void`. A listener that failed to register would
      // then crash the cleanup instead of no-op'ing, and `tsc` rejects it.
    }).catch(() => () => {});
    return () => {
      disposed = true;
      stop.then((f) => f()).catch(() => {});
    };
  }, []);

  return (
    <ChatContext.Provider value={value}>
      {children}
    </ChatContext.Provider>
  );
}
