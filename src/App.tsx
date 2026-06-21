import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import "./App.css";

interface ConversationEntry {
  id: string;
  peer_key_hex: string;
  display_name: string | null;
  peer_display_name: string | null;
  last_message_at: number | null;
  last_message_preview: string | null;
  message_count: number;
  is_online: boolean;
  auto_delete_at: number | null;
  retention_policy: string;
  created_at: number;
}

interface IdentityInfo {
  fingerprint: string;
  public_key_hex: string;
  has_identity: boolean;
}

interface ChatMessage {
  id: string;
  content: string;
  direction: string;
  timestamp: number;
}

interface ConnectionInfo {
  state: string;
  peer_fingerprint: string | null;
  peer_verified: boolean;
  peer_key_hex: string | null;
}

interface FileRequest {
  peer_key_hex: string;
  transfer_id: string;
  filename: string;
  total_size: number;
}

interface VaultStatus {
  initialized: boolean;
  unlocked: boolean;
}

interface NetworkSettings {
  tor_enabled: boolean;
  tor_proxy_addr: string;
  tor_reachable: boolean;
  public_ip: string | null;
}

function App() {
  const [view, setView] = useState<
    "setup" | "vault" | "hub" | "chat" | "settings"
  >("setup");
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const [connection, setConnection] = useState<ConnectionInfo | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputText, setInputText] = useState("");
  const [inviteToConnect, setInviteToConnect] = useState("");
  const [generatedInvite, setGeneratedInvite] = useState("");
  const [copied, setCopied] = useState(false);
  const [fileRequests, setFileRequests] = useState<FileRequest[]>([]);
  const [isConnecting, setIsConnecting] = useState(false);

  // Vault state
  const [passphrase, setPassphrase] = useState("");
  const [passphraseConfirm, setPassphraseConfirm] = useState("");
  const [vaultError, setVaultError] = useState("");
  const [vaultInitialized, setVaultInitialized] = useState(false);

  // Settings state
  const [networkSettings, setNetworkSettings] =
    useState<NetworkSettings | null>(null);
  const [publicIp, setPublicIp] = useState<string | null>(null);
  const [stunLoading, setStunLoading] = useState(false);

  // Multi-conversation state
  const [conversations, setConversations] = useState<ConversationEntry[]>([]);
  const [hubTab, setHubTab] = useState<"connect" | "chats">("connect");
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null);

  // Naming state (for invite validation)
  const [inviteValid, setInviteValid] = useState(false);
  const [namingMyName, setNamingMyName] = useState("");
  const [namingTheirName, setNamingTheirName] = useState("");

  // Per-conversation retention
  const [retentionPolicy, setRetentionPolicy] = useState("none");
  const [retentionDuration, setRetentionDuration] = useState<string>("86400");

  // Notification permission
  const [notifPermission, setNotifPermission] = useState(false);

  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Request notification permission on mount
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

  // Initialize and check identity
  useEffect(() => {
    async function checkIdentity() {
      try {
        const info = await invoke<IdentityInfo>("init_identity");
        setIdentity(info);
        if (info.has_identity) {
          // Existing identity — check vault status
          const vs = await invoke<VaultStatus>("get_vault_status");
          setVaultInitialized(vs.initialized);
          if (vs.unlocked) {
            setView("hub");
          } else {
            // Show vault screen ("set passphrase" for legacy, "enter passphrase" for vaulted)
            setView("vault");
          }
        } else {
          // No identity — go directly to vault setup to create one
          setVaultInitialized(false);
          setView("vault");
        }
      } catch (err) {
        console.error("Init failed:", err);
      }
    }
    checkIdentity();
  }, []);

  // Event listeners
  useEffect(() => {
    const unlistenMsg = listen<any>("m2m://message", (event) => {
      setMessages((prev) => [...prev, event.payload.message]);
      // Desktop notification for received messages
      if (notifPermission && event.payload.message.direction === "received") {
        sendNotification({
          title: "M2M — New Message",
          body: event.payload.message.content.slice(0, 100),
        });
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
          const history = await invoke<ChatMessage[]>("load_messages", {
            peerKeyHex: event.payload.peer_key_hex,
          });
          setMessages(history);
        } catch (e) {
          console.error("Failed to load history", e);
        }
        if (notifPermission) {
          sendNotification({
            title: "M2M — Peer Connected",
            body: `Encrypted session established`,
          });
        }
      } else if (stateStr === "disconnected") {
        setView("hub");
        setConnection(null);
        setMessages([]);
        setActiveConversationId(null);
      }
      // Refresh conversation list
      try { const c = await invoke<ConversationEntry[]>("list_conversations"); setConversations(c); } catch {}
    });

    const unlistenConvMeta = listen<any>("m2m://conversation-meta", async () => {
      try { const c = await invoke<ConversationEntry[]>("list_conversations"); setConversations(c); } catch {}
    });

    const unlistenFileReq = listen<any>("m2m://file-request", (event) => {
      setFileRequests((prev) => [...prev, event.payload]);
      if (notifPermission) {
        sendNotification({
          title: "M2M — File Transfer",
          body: `Incoming file: ${event.payload.filename}`,
        });
      }
    });

    const unlistenFileComp = listen<any>("m2m://file-complete", (event) => {
      if (notifPermission) {
        sendNotification({
          title: "M2M — File Received",
          body: `Saved to: ${event.payload.path}`,
        });
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

  // Fetch conversations
  const loadConversations = async () => {
    try {
      const c = await invoke<ConversationEntry[]>("list_conversations");
      setConversations(c);
    } catch (e) {
      console.error("Failed to load conversations", e);
    }
  };

  useEffect(() => {
    if (view === "hub") {
      loadConversations();
    }
  }, [view]);

  // Validate invite input
  useEffect(() => {
    if (inviteToConnect.length > 30) {
      invoke<any>("validate_invite", { inviteStr: inviteToConnect })
        .then((info) => {
          if (info.valid) setInviteValid(true);
        })
        .catch(() => setInviteValid(false));
    } else {
      setInviteValid(false);
    }
  }, [inviteToConnect]);

  // Auto-scroll messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleUnlockVault = async () => {
    setVaultError("");
    if (passphrase.length < 8) {
      setVaultError("Passphrase must be at least 8 characters.");
      return;
    }
    // Require confirmation only on first-time setup
    if (!vaultInitialized && passphraseConfirm && passphrase !== passphraseConfirm) {
      setVaultError("Passphrases do not match.");
      return;
    }
    if (!vaultInitialized && !passphraseConfirm) {
      setVaultError("Please confirm your passphrase.");
      return;
    }
    try {
      await invoke("unlock_vault", { passphrase });
      // Refresh identity info after unlock (keypair is now loaded)
      const info = await invoke<IdentityInfo>("get_identity");
      setIdentity(info);
      setView("hub");
    } catch (e: any) {
      setVaultError(String(e));
    }
  };

  const handleGenerateInvite = async () => {
    try {
      await invoke("start_listening", { address: "0.0.0.0:0" });
      const address = await invoke<string>("get_listen_address");
      const invite = await invoke<string>("create_invite", {
        address,
        validityMinutes: 60,
        oneTime: true,
      });
      setGeneratedInvite(invite);
    } catch (e) {
      console.error(e);
    }
  };

  const copyInvite = () => {
    navigator.clipboard.writeText(generatedInvite);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleConnect = async () => {
    if (!inviteToConnect) return;
    setIsConnecting(true);
    try {
      const info = await invoke<ConnectionInfo>("connect_to_peer", {
        inviteStr: inviteToConnect,
      });
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
        const history = await invoke<ChatMessage[]>("load_messages", {
          peerKeyHex: info.peer_key_hex,
        });
        setMessages(history);
      } catch (e) {
        console.error("Failed to load history", e);
      }
    } catch (e) {
      console.error("Connection failed", e);
      alert("Connection failed: " + e);
    } finally {
      setIsConnecting(false);
    }
  };

  const handleOpenChat = async (conv: ConversationEntry) => {
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
      const history = await invoke<ChatMessage[]>("load_messages", {
        peerKeyHex: conv.peer_key_hex,
      });
      setMessages(history);
    } catch (e) {
      console.error("Failed to load history", e);
    }
  };

  const handleSendMessage = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!inputText.trim() || !connection?.peer_key_hex) return;
    try {
      const msg = await invoke<ChatMessage>("send_message", {
        peerKeyHex: connection.peer_key_hex,
        content: inputText,
      });
      setMessages((prev) => [...prev, msg]);
      setInputText("");
    } catch (e) {
      console.error(e);
    }
  };

  const handleVerify = async () => {
    if (!connection?.peer_key_hex) return;
    try {
      await invoke("verify_peer", { peerKeyHex: connection.peer_key_hex });
      setConnection({ ...connection, peer_verified: true });
    } catch (e) {
      console.error(e);
    }
  };

  const handleDisconnect = async () => {
    if (!connection?.peer_key_hex) return;
    try {
      await invoke("disconnect_peer", { peerKeyHex: connection.peer_key_hex });
      setView("hub");
      setConnection(null);
      setMessages([]);
    } catch (e) {
      console.error("Disconnect failed", e);
    }
  };

  // Native Tauri dialog for file selection
  const handleSendFile = async () => {
    if (!connection?.peer_key_hex) return;
    try {
      const selected = await open({
        multiple: false,
        title: "Select file to send",
      });
      if (!selected) return;
      const filePath = typeof selected === "string" ? selected : selected;
      await invoke("send_file", {
        peerKeyHex: connection.peer_key_hex,
        filePath: filePath,
      });
      const filename =
        typeof filePath === "string"
          ? filePath.split(/[\\/]/).pop()
          : "file";
      setMessages((prev) => [
        ...prev,
        {
          id: Date.now().toString(),
          content: `📎 File request sent: ${filename}`,
          direction: "sent",
          timestamp: Math.floor(Date.now() / 1000),
        },
      ]);
    } catch (e) {
      alert("Failed to send file: " + e);
    }
  };

  // Native Tauri dialog for save location
  const acceptFile = async (req: FileRequest) => {
    try {
      const filePath = await save({
        title: `Save "${req.filename}" to...`,
        defaultPath: req.filename,
      });
      if (!filePath) return;
      // Pass the full file path — backend handles dir vs file detection
      await invoke("accept_file_transfer", {
        peerKeyHex: req.peer_key_hex,
        transferId: req.transfer_id,
        saveDir: filePath,
      });
      setFileRequests((prev) =>
        prev.filter((r) => r.transfer_id !== req.transfer_id)
      );
    } catch (err) {
      alert("Accept failed: " + err);
    }
  };

  const rejectFile = async (req: FileRequest) => {
    try {
      await invoke("reject_file_transfer", {
        peerKeyHex: req.peer_key_hex,
        transferId: req.transfer_id,
      });
      setFileRequests((prev) =>
        prev.filter((r) => r.transfer_id !== req.transfer_id)
      );
    } catch (err) {
      alert("Reject failed: " + err);
    }
  };

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  };

  // Settings helpers
  const openSettings = async () => {
    setView("settings");
    try {
      const ns = await invoke<NetworkSettings>("get_network_settings");
      setNetworkSettings(ns);
      setPublicIp(ns.public_ip);
    } catch (e) {
      console.error("Failed to load network settings", e);
    }
  };

  const handleStunDiscover = async () => {
    setStunLoading(true);
    try {
      const ip = await invoke<string>("discover_public_ip");
      setPublicIp(ip);
    } catch (e) {
      alert("STUN failed: " + e);
    } finally {
      setStunLoading(false);
    }
  };

  const handleTorToggle = async () => {
    if (!networkSettings) return;
    const newVal = !networkSettings.tor_enabled;
    try {
      await invoke("set_tor_enabled", { enabled: newVal });
      setNetworkSettings({ ...networkSettings, tor_enabled: newVal });
    } catch (e) {
      alert("Tor toggle failed: " + e);
    }
  };

  // ═══════════ Setup View ═══════════
  if (view === "setup") {
    return (
      <div className="app-container">
        <div className="centered-view">
          <div className="setup-icon">🔑</div>
          <h2>Initializing Secure Enclave</h2>
          <p>
            Generating Ed25519 identity keys.
            <br />
            They never leave your device.
          </p>
          <div className="loading-dots">
            <span />
            <span />
            <span />
          </div>
        </div>
      </div>
    );
  }

  // ═══════════ Vault Unlock View ═══════════
  if (view === "vault") {
    const isFirstTime = !vaultInitialized;
    return (
      <div className="app-container">
        <div className="centered-view">
          <div className="setup-icon vault-icon">🔐</div>
          <h2>{isFirstTime ? "Set Up Your Vault" : "Unlock Your Vault"}</h2>
          <p>
            {isFirstTime
              ? "Choose a passphrase to encrypt your local data. This protects your identity keys and message history."
              : "Enter your passphrase to decrypt your local data."}
            <br />
            Minimum 8 characters. Uses Argon2id key derivation.
          </p>
          <div className="vault-form">
            <input
              id="vault-passphrase"
              type="password"
              placeholder="Passphrase"
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleUnlockVault()}
            />
            {isFirstTime && (
              <input
                id="vault-passphrase-confirm"
                type="password"
                placeholder="Confirm passphrase"
                value={passphraseConfirm}
                onChange={(e) => setPassphraseConfirm(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleUnlockVault()}
              />
            )}
            {vaultError && <div className="vault-error">{vaultError}</div>}
            <button id="vault-unlock-btn" onClick={handleUnlockVault}>
              {isFirstTime ? "Create Vault" : "Unlock"}
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ═══════════ Settings View ═══════════
  if (view === "settings") {
    return (
      <div className="app-container">
        <div className="header">
          <h1>
            <span>⚙️</span> Settings
          </h1>
          <button
            className="secondary"
            onClick={() => setView("hub")}
            id="back-to-hub-btn"
          >
            ← Back
          </button>
        </div>
        <div className="content-area settings-content">
          {/* Network / STUN */}
          <div className="settings-section">
            <h3>Network</h3>
            <div className="settings-row">
              <div className="settings-label">
                <strong>Public IP</strong>
                <span className="settings-desc">
                  Discovered via STUN — needed for invites that work across the
                  internet.
                </span>
              </div>
              <div className="settings-value">
                {publicIp ? (
                  <span className="mono-value">{publicIp}</span>
                ) : (
                  <span className="text-muted">Not discovered</span>
                )}
                <button
                  className="secondary"
                  onClick={handleStunDiscover}
                  disabled={stunLoading}
                  id="stun-discover-btn"
                >
                  {stunLoading ? "..." : "Discover"}
                </button>
              </div>
            </div>

            {/* Tor */}
            <div className="settings-row">
              <div className="settings-label">
                <strong>Tor Routing</strong>
                <span className="settings-desc">
                  Route all outgoing connections through Tor SOCKS5 proxy
                  (127.0.0.1:9050).
                </span>
              </div>
              <div className="settings-value">
                <span
                  className={`tor-status ${networkSettings?.tor_reachable ? "reachable" : "unreachable"}`}
                >
                  {networkSettings?.tor_reachable ? "Proxy reachable" : "Proxy not found"}
                </span>
                <button
                  className={networkSettings?.tor_enabled ? "danger" : "secondary"}
                  onClick={handleTorToggle}
                  id="tor-toggle-btn"
                >
                  {networkSettings?.tor_enabled ? "Disable Tor" : "Enable Tor"}
                </button>
              </div>
            </div>
          </div>

          {/* Identity */}
          <div className="settings-section">
            <h3>Identity</h3>
            <div className="fingerprint-box" id="settings-fingerprint">
              <span className="fingerprint-label">Your Identity Fingerprint</span>
              {identity?.fingerprint}
            </div>
          </div>
        </div>
      </div>
    );
  }

  // ═══════════ Hub View ═══════════
  if (view === "hub") {
    return (
      <div className="app-container">
        <div className="header">
          <h1>
            <span>🛡️</span> M2M
          </h1>
          <div className="header-actions">
            <div className="status-badge">Offline</div>
            <button
              className="icon-btn"
              onClick={openSettings}
              title="Settings"
              id="settings-btn"
            >
              ⚙️
            </button>
          </div>
        </div>

        <div className="hub-tabs">
          <button
            className={`hub-tab ${hubTab === "connect" ? "active" : ""}`}
            onClick={() => setHubTab("connect")}
          >
            🔌 Connect
          </button>
          <button
            className={`hub-tab ${hubTab === "chats" ? "active" : ""}`}
            onClick={() => setHubTab("chats")}
          >
            💬 Chats
            {conversations.length > 0 && <span className="tab-badge">{conversations.length}</span>}
          </button>
        </div>

        <div className="content-area hub-tab-content">
          {hubTab === "connect" && (
            <div className="centered-view">
              <div className="invite-section">
                {/* Host Card */}
                <div className="card" id="host-card">
                  <div className="card-header">
                    <div className="card-icon host">➕</div>
                    <h3>Host a Connection</h3>
                  </div>
                  <p className="card-desc">
                    Generate a one-time signed invite for a peer to connect to you
                    securely.
                  </p>
                  {!generatedInvite ? (
                    <button id="generate-invite-btn" onClick={handleGenerateInvite}>
                      Generate Invite Link
                    </button>
                  ) : (
                    <div className="invite-output">
                      <input readOnly value={generatedInvite} id="invite-output" />
                      <button
                        className="icon-btn"
                        onClick={copyInvite}
                        title="Copy to clipboard"
                        id="copy-invite-btn"
                      >
                        {copied ? "✓" : "📋"}
                      </button>
                    </div>
                  )}
                </div>

                {/* Join Card */}
                <div className="card" id="join-card">
                  <div className="card-header">
                    <div className="card-icon join">🔗</div>
                    <h3>Join a Connection</h3>
                  </div>
                  <p className="card-desc">
                    Paste an invite link from a trusted peer to connect.
                  </p>
                  <div className="flex-row">
                    <input
                      id="invite-input"
                      placeholder="m2m://..."
                      value={inviteToConnect}
                      onChange={(e) => setInviteToConnect(e.target.value)}
                    />
                    <button
                      id="connect-btn"
                      onClick={handleConnect}
                      disabled={isConnecting || !inviteToConnect}
                    >
                      {isConnecting ? "..." : "Connect"}
                    </button>
                  </div>
                  {inviteValid && (
                    <div className="naming-panel">
                      <div className="valid-badge">✅ Valid Invite Found</div>
                      <label>
                        Your Display Name (optional)
                        <input
                          placeholder="How they will see you"
                          value={namingMyName}
                          onChange={(e) => setNamingMyName(e.target.value)}
                        />
                      </label>
                      <label>
                        Their Display Name (optional)
                        <input
                          placeholder="How you want to see them"
                          value={namingTheirName}
                          onChange={(e) => setNamingTheirName(e.target.value)}
                        />
                      </label>
                    </div>
                  )}
                </div>

                <div className="section-divider" />

                {/* Fingerprint */}
                <div className="fingerprint-box" id="fingerprint-display">
                  <span className="fingerprint-label">Your Identity Fingerprint</span>
                  {identity?.fingerprint}
                </div>
              </div>
            </div>
          )}

          {hubTab === "chats" && (
            <div className="conversation-list">
              {conversations.length === 0 ? (
                <div className="conversation-list-empty">
                  <span className="empty-icon">📭</span>
                  No conversations yet. Connect to a peer to start chatting!
                </div>
              ) : (
                conversations.map((c) => (
                  <div key={c.id} className="conversation-item" onClick={() => handleOpenChat(c)}>
                    <div className={`conv-avatar ${c.is_online ? "online" : ""}`}>
                      {(c.display_name || c.peer_display_name || c.peer_key_hex).charAt(0)}
                    </div>
                    <div className="conv-body">
                      <div className="conv-top-row">
                        <span className="conv-name">
                          {c.display_name || c.peer_display_name || "Unknown Peer"}
                        </span>
                        {c.last_message_at && (
                          <span className="conv-time">
                            {new Date(c.last_message_at * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                          </span>
                        )}
                      </div>
                      <div className="conv-preview">
                        {c.last_message_preview || "No messages yet."}
                      </div>
                      <div className="conv-retention-badge">
                        {c.retention_policy !== "none" && `Policy: ${c.retention_policy}`}
                      </div>
                    </div>
                    <div className={`conv-status-dot ${c.is_online ? "online" : "offline"}`} />
                    <div className="conv-actions">
                      <button 
                        className="danger" 
                        onClick={(e) => { 
                          e.stopPropagation(); 
                          invoke("delete_conversation_cmd", { conversationId: c.id })
                            .then(loadConversations)
                            .catch(console.error);
                        }}
                      >
                        Delete
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>
          )}
        </div>
      </div>
    );
  }

  // ═══════════ Chat View ═══════════
  return (
    <div className="app-container">
      <div className="header">
        <h1>
          {connection?.peer_verified ? (
            <span style={{ fontSize: "1rem" }}>✅</span>
          ) : (
            <span
              className="verify-btn"
              onClick={handleVerify}
              title="Click to verify peer fingerprint"
              style={{ fontSize: "1rem" }}
            >
              ⚠️
            </span>
          )}
          Encrypted Session
        </h1>
        <div className="header-actions">
          <button className="secondary" onClick={() => setView("hub")}>
            ← Hub
          </button>
          <div
            className={`status-badge ${
              connection?.state === "established"
                ? "connected"
                : "disconnected"
            }`}
          >
            {connection?.state || "unknown"}
          </div>
          {connection?.state === "established" && (
            <button
              className="danger"
              onClick={handleDisconnect}
              id="disconnect-btn"
            >
              Disconnect
            </button>
          )}
        </div>
      </div>

      {/* File Transfer Requests */}
      {fileRequests.length > 0 && (
        <div className="file-requests">
          {fileRequests.map((req) => (
            <div key={req.transfer_id} className="file-request-banner">
              <div className="file-info">
                <div className="file-icon">📄</div>
                <div>
                  <strong>{req.filename}</strong>
                  <br />
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
                    {formatSize(req.total_size)}
                  </span>
                </div>
              </div>
              <div className="file-actions">
                <button onClick={() => acceptFile(req)}>Accept</button>
                <button className="secondary" onClick={() => rejectFile(req)}>
                  Reject
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Messages */}
      <div className="messages" id="message-list">
        <div className="session-banner">
          <div className="lock-icon">🔒</div>
          <p>
            End-to-end encrypted session established.
            <br />
            <span className="peer-fp">
              {connection?.peer_fingerprint || activeConversationId}
            </span>
          </p>
        </div>

        {activeConversationId && (
          <div className="retention-config">
            <h4>Conversation Policy</h4>
            <div className="retention-row">
              <select 
                value={retentionPolicy} 
                onChange={(e) => {
                  setRetentionPolicy(e.target.value);
                  invoke("set_conversation_retention", {
                    conversationId: activeConversationId,
                    policy: e.target.value,
                    durationSecs: e.target.value === "none" ? null : parseInt(retentionDuration, 10),
                  }).catch(console.error);
                }}
              >
                <option value="none">No Expiration</option>
                <option value="delete">Auto-Delete After</option>
                <option value="export">Auto-Export After</option>
              </select>
              {retentionPolicy !== "none" && (
                <select
                  value={retentionDuration}
                  onChange={(e) => {
                    setRetentionDuration(e.target.value);
                    invoke("set_conversation_retention", {
                      conversationId: activeConversationId,
                      policy: retentionPolicy,
                      durationSecs: parseInt(e.target.value, 10),
                    }).catch(console.error);
                  }}
                >
                  <option value="3600">1 Hour</option>
                  <option value="86400">24 Hours</option>
                  <option value="604800">7 Days</option>
                </select>
              )}
              <button 
                className="secondary" 
                onClick={async () => {
                  try {
                    const savePath = await save({
                      title: "Export Conversation",
                      defaultPath: `export_${activeConversationId}.json`
                    });
                    if (savePath) {
                      await invoke("export_conversation", {
                        conversationId: activeConversationId,
                        exportPath: savePath
                      });
                      alert("Exported successfully to " + savePath);
                    }
                  } catch (e) {
                    alert("Export failed: " + e);
                  }
                }}
              >
                Export Now
              </button>
            </div>
          </div>
        )}

        {messages.map((m) => (
          <div key={m.id} className={`message-bubble ${m.direction}`}>
            {m.content}
            <span className="message-time">
              {new Date(m.timestamp * 1000).toLocaleTimeString([], {
                hour: "2-digit",
                minute: "2-digit",
              })}
            </span>
          </div>
        ))}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <form className="input-area" onSubmit={handleSendMessage}>
        <button
          type="button"
          className="icon-btn"
          onClick={handleSendFile}
          title="Send a file"
          id="send-file-btn"
        >
          📎
        </button>
        <input
          id="message-input"
          placeholder="Type a secure message..."
          value={inputText}
          onChange={(e) => setInputText(e.target.value)}
          autoFocus
        />
        <button type="submit" className="send-btn" id="send-message-btn">
          ➤
        </button>
      </form>
    </div>
  );
}

export default App;
