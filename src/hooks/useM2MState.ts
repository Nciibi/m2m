import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { useToast } from "./useToast";
import type {
  IdentityInfo,
  ConnectionInfo,
  ChatMessage,
  FileRequest,
  ConversationEntry,
  VaultStatus,
  NetworkSettings,
  StunConfig,
  NatTypeInfo,
} from "../types";

export type ViewName = "setup" | "vault" | "hub" | "chat" | "settings";

export function useM2MState() {
  const { toasts, addToast, removeToast } = useToast();

  // ─── Core State ───
  const [view, setView] = useState<ViewName>("setup");
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const [connection, setConnection] = useState<ConnectionInfo | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [fileRequests, setFileRequests] = useState<FileRequest[]>([]);
  const [isConnecting, setIsConnecting] = useState(false);

  // Vault state
  const [vaultInitialized, setVaultInitialized] = useState(false);

  // Settings state
  const [networkSettings, setNetworkSettings] = useState<NetworkSettings | null>(null);
  const [publicIp, setPublicIp] = useState<string | null>(null);
  const [stunLoading, setStunLoading] = useState(false);
  const [networkDiagnostics, setNetworkDiagnostics] = useState<NatTypeInfo | null>(null);
  const [stunConfig, setStunConfig] = useState<StunConfig | null>(null);
  const [stunServerInput, setStunServerInput] = useState("");
  const [privateMode, setPrivateMode] = useState(false);
  const [connectivityResult, setConnectivityResult] = useState<any>(null);

  // Multi-conversation state
  const [conversations, setConversations] = useState<ConversationEntry[]>([]);
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null);

  // Naming state
  const [inviteToConnect, setInviteToConnect] = useState("");
  const [inviteValid, setInviteValid] = useState(false);
  const [namingMyName, setNamingMyName] = useState("");
  const [namingTheirName, setNamingTheirName] = useState("");

  // Invite generation
  const [generatedInvite, setGeneratedInvite] = useState("");

  // Retention
  const [retentionPolicy, setRetentionPolicy] = useState("none");
  const [retentionDuration, setRetentionDuration] = useState<string>("86400");

  // Notification permission
  const [notifPermission, setNotifPermission] = useState(false);

  // Theme
  const [theme, setTheme] = useState<"dark" | "light">("dark");

  // ==================== Theme ====================

  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    const update = (e: MediaQueryListEvent | MediaQueryList) => {
      setTheme(e.matches ? "light" : "dark");
    };
    update(mq);
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  // ==================== Handlers ====================

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
    } catch (e) {
      console.error(e);
    }
  }, [connection]);

  const handleDisconnect = useCallback(async () => {
    if (!connection?.peer_key_hex) return;
    try {
      await invoke("disconnect_peer", { peerKeyHex: connection.peer_key_hex });
      setView("hub");
      setConnection(null);
      setMessages([]);
    } catch (e) {
      console.error("Disconnect failed", e);
    }
  }, [connection?.peer_key_hex]);

  const handleSendFile = useCallback(async () => {
    if (!connection?.peer_key_hex) return;
    try {
      const selected = await open({ multiple: false, title: "Select file to send" });
      if (!selected) return;
      const filePath = typeof selected === "string" ? selected : selected;
      await invoke("send_file", { peerKeyHex: connection.peer_key_hex, filePath });
      const filename = typeof filePath === "string" ? filePath.split(/[\\/]/).pop() : "file";
      setMessages((prev) => [
        ...prev,
        {
          id: Date.now().toString(),
          content: `File request sent: ${filename}`,
          direction: "sent",
          timestamp: Math.floor(Date.now() / 1000),
        },
      ]);
    } catch (e) {
      addToast("Failed to send file: " + e, "error");
    }
  }, [connection?.peer_key_hex, addToast]);

  const handleExportConversation = useCallback(async () => {
    if (!activeConversationId) return;
    try {
      const savePath = await save({
        title: "Export Conversation",
        defaultPath: `export_${activeConversationId}.json`,
      });
      if (savePath) {
        await invoke("export_conversation", {
          conversationId: activeConversationId,
          exportPath: savePath,
        });
        addToast("Exported successfully to " + savePath, "success");
      }
    } catch (e) {
      addToast("Export failed: " + e, "error");
    }
  }, [activeConversationId, addToast]);

  const handleSetRetention = useCallback(async (policy: string, durationSecs: number | null) => {
    if (!activeConversationId) return;
    try {
      await invoke("set_conversation_retention", {
        conversationId: activeConversationId,
        policy,
        durationSecs,
      });
    } catch (e) {
      console.error("Failed to set retention", e);
    }
  }, [activeConversationId]);

  const openSettings = useCallback(async () => {
    setView("settings");
    try {
      const ns = await invoke<NetworkSettings>("get_network_settings");
      setNetworkSettings(ns);
      setPublicIp(ns.public_ip);
      const sc = await invoke<StunConfig>("get_stun_config");
      setStunConfig(sc);
      setPrivateMode(sc.private_mode);
      try {
        const diag = await invoke<NatTypeInfo>("get_network_diagnostics");
        setNetworkDiagnostics(diag);
      } catch (e) {
        console.error("Failed to load diagnostics", e);
      }
    } catch (e) {
      console.error("Failed to load network settings", e);
    }
  }, []);

  const handleUnlockVault = useCallback(async (passphrase: string) => {
    await invoke("unlock_vault", { passphrase });
    const info = await invoke<IdentityInfo>("get_identity");
    setIdentity(info);
    setView("hub");
  }, []);

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
          peerKeyHex: info.peer_key_hex,
          myName: namingMyName,
          theirName: namingTheirName,
        }).catch(console.error);
      }
      setView("chat");
      try {
        const history = await invoke<ChatMessage[]>("load_messages", { peerKeyHex: info.peer_key_hex });
        setMessages(history);
      } catch (e) {
        console.error("Failed to load history", e);
      }
    } catch (e) {
      console.error("Connection failed", e);
      addToast("Connection failed: " + e, "error");
    } finally {
      setIsConnecting(false);
    }
  }, [inviteToConnect, namingMyName, namingTheirName, addToast]);

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
      const history = await invoke<ChatMessage[]>("load_messages", { peerKeyHex: conv.peer_key_hex });
      setMessages(history);
    } catch (e) {
      console.error("Failed to load history", e);
    }
  }, []);

  const handleStunDiscover = useCallback(async () => {
    setStunLoading(true);
    try {
      const ip = await invoke<string>("discover_public_ip");
      setPublicIp(ip);
      const diag = await invoke<NatTypeInfo>("get_network_diagnostics");
      setNetworkDiagnostics(diag);
    } catch (e) {
      addToast("STUN failed: " + e, "error");
    } finally {
      setStunLoading(false);
    }
  }, [addToast]);

  const handleAddStunServer = useCallback(async () => {
    if (!stunConfig || !stunServerInput.trim()) return;
    const newServers = [...stunConfig.servers, stunServerInput.trim()];
    try {
      await invoke("set_stun_servers", { servers: newServers });
      setStunConfig({ ...stunConfig, servers: newServers });
      setStunServerInput("");
    } catch (e) {
      addToast("Failed to add STUN server: " + e, "error");
    }
  }, [stunConfig, stunServerInput, addToast]);

  const handleRemoveStunServer = useCallback(async (idx: number) => {
    if (!stunConfig) return;
    const newServers = stunConfig.servers.filter((_, i) => i !== idx);
    if (newServers.length === 0) {
      addToast("Cannot remove all STUN servers — at least one required.", "warning");
      return;
    }
    try {
      await invoke("set_stun_servers", { servers: newServers });
      setStunConfig({ ...stunConfig, servers: newServers });
    } catch (e) {
      addToast("Failed to remove STUN server: " + e, "error");
    }
  }, [stunConfig, addToast]);

  const handleResetStunDefaults = useCallback(async () => {
    const defaults = ["stun.l.google.com:19302", "stun1.l.google.com:19302", "stun.cloudflare.com:3478", "stun.nextcloud.com:3478"];
    try {
      await invoke("set_stun_servers", { servers: defaults });
      setStunConfig(stunConfig ? { ...stunConfig, servers: defaults } : null);
    } catch (e) {
      addToast("Failed to reset STUN servers: " + e, "error");
    }
  }, [stunConfig, addToast]);

  const handlePrivateModeToggle = useCallback(async () => {
    const newVal = !privateMode;
    try {
      await invoke("set_private_mode", { enabled: newVal });
      setPrivateMode(newVal);
    } catch (e) {
      console.error("Failed to set private mode:", e);
    }
  }, [privateMode]);

  const handleConnectivityCheck = useCallback(async () => {
    try {
      const result = await invoke<any>("check_connectivity");
      setConnectivityResult(result);
      const diag = await invoke<NatTypeInfo>("get_network_diagnostics");
      setNetworkDiagnostics(diag);
    } catch (e) {
      addToast("Connectivity check failed: " + e, "error");
    }
  }, [addToast]);

  const handleTorToggle = useCallback(async () => {
    if (!networkSettings) return;
    const newVal = !networkSettings.tor_enabled;
    try {
      await invoke("set_tor_enabled", { enabled: newVal });
      setNetworkSettings({ ...networkSettings, tor_enabled: newVal });
    } catch (e) {
      addToast("Tor toggle failed: " + e, "error");
    }
  }, [networkSettings, addToast]);

  const loadConversations = useCallback(async () => {
    try {
      const c = await invoke<ConversationEntry[]>("list_conversations");
      setConversations(c);
    } catch (e) {
      console.error("Failed to load conversations", e);
    }
  }, []);

  const handleDeleteConversation = useCallback(() => {
    loadConversations();
  }, [loadConversations]);

  // ==================== Effects ====================

  useEffect(() => {
    async function setupNotifications() {
      let granted = await isPermissionGranted();
      if (!granted) {
        const result = await requestPermission();
        granted = result === "granted";
      }
      setNotifPermission(granted);
    }
    setupNotifications();
  }, []);

  useEffect(() => {
    async function checkIdentity() {
      try {
        const info = await invoke<IdentityInfo>("init_identity");
        setIdentity(info);
        if (info.has_identity) {
          const vs = await invoke<VaultStatus>("get_vault_status");
          setVaultInitialized(vs.initialized);
          setView(vs.unlocked ? "hub" : "vault");
        } else {
          setVaultInitialized(false);
          setView("vault");
        }
      } catch (err) {
        console.error("Init failed:", err);
      }
    }
    checkIdentity();
  }, []);

  useEffect(() => {
    const unlistenMsg = listen<any>("m2m://message", (event) => {
      setMessages((prev) => [...prev, event.payload.message]);
      if (notifPermission && event.payload.message.direction === "received") {
        sendNotification({ title: "M2M — New Message", body: event.payload.message.content.slice(0, 100) });
      }
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
          const history = await invoke<ChatMessage[]>("load_messages", { peerKeyHex: event.payload.peer_key_hex });
          setMessages(history);
        } catch (e) {
          console.error("Failed to load history", e);
        }
        if (notifPermission) {
          sendNotification({ title: "M2M — Peer Connected", body: "Encrypted session established" });
        }
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
      if (notifPermission) {
        sendNotification({ title: "M2M — File Transfer", body: `Incoming file: ${event.payload.filename}` });
      }
    });

    const unlistenFileComp = listen<any>("m2m://file-complete", (event) => {
      if (notifPermission) {
        sendNotification({ title: "M2M — File Received", body: `Saved to: ${event.payload.path}` });
      }
    });

    return () => {
      unlistenMsg.then((f) => f());
      unlistenConn.then((f) => f());
      unlistenFileReq.then((f) => f());
      unlistenFileComp.then((f) => f());
      unlistenConvMeta.then((f) => f());
    };
  }, [notifPermission]);

  useEffect(() => {
    if (view === "hub") loadConversations();
  }, [view, loadConversations]);

  useEffect(() => {
    if (inviteToConnect.length > 30) {
      invoke<any>("validate_invite", { inviteStr: inviteToConnect })
        .then((info) => { if (info.valid) setInviteValid(true); })
        .catch(() => setInviteValid(false));
    } else {
      setInviteValid(false);
    }
  }, [inviteToConnect]);

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape" && view === "chat") { e.preventDefault(); setView("hub"); }
      if ((e.ctrlKey || e.metaKey) && e.key === ",") { e.preventDefault(); if (view !== "settings") openSettings(); }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [view, openSettings]);

  return {
    // Toast
    toasts, addToast, removeToast,
    // View navigation
    view, setView,
    // Identity
    identity,
    // Connection
    connection, isConnecting,
    // Messages
    messages, fileRequests,
    // Vault
    vaultInitialized, handleUnlockVault,
    // Settings data
    networkSettings, publicIp, stunLoading, networkDiagnostics,
    stunConfig, stunServerInput, setStunServerInput,
    privateMode, connectivityResult,
    // Conversations
    conversations, activeConversationId,
    // Naming
    inviteToConnect, setInviteToConnect,
    inviteValid, namingMyName, setNamingMyName,
    namingTheirName, setNamingTheirName,
    // Invite
    generatedInvite,
    // Retention
    retentionPolicy, setRetentionPolicy,
    retentionDuration, setRetentionDuration,
    // Handlers
    handleSendMessage, handleVerify, handleDisconnect,
    handleSendFile, handleExportConversation, handleSetRetention,
    openSettings, handleGenerateInvite, copyInvite,
    handleConnect, handleOpenChat,
    handleStunDiscover, handleAddStunServer,
    handleRemoveStunServer, handleResetStunDefaults,
    handlePrivateModeToggle, handleConnectivityCheck,
    handleTorToggle, handleDeleteConversation,
  };
}
