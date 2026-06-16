import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

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

function App() {
  const [view, setView] = useState<"setup" | "hub" | "chat">("setup");
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const [connection, setConnection] = useState<ConnectionInfo | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputText, setInputText] = useState("");
  const [inviteToConnect, setInviteToConnect] = useState("");
  const [generatedInvite, setGeneratedInvite] = useState("");
  const [copied, setCopied] = useState(false);
  const [fileRequests, setFileRequests] = useState<FileRequest[]>([]);
  const [isConnecting, setIsConnecting] = useState(false);

  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Initialize and check identity
  useEffect(() => {
    async function checkIdentity() {
      try {
        const info = await invoke<IdentityInfo>("init_identity");
        setIdentity(info);
        if (info.has_identity) {
          setView("hub");
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
        setView("chat");
        try {
          const history = await invoke<ChatMessage[]>("load_messages", {
            peerKeyHex: event.payload.peer_key_hex,
          });
          setMessages(history);
        } catch (e) {
          console.error("Failed to load history", e);
        }
      } else if (stateStr === "disconnected") {
        setView("hub");
        setConnection(null);
        setMessages([]);
      }
    });

    const unlistenFileReq = listen<any>("m2m://file-request", (event) => {
      setFileRequests((prev) => [...prev, event.payload]);
    });

    const unlistenFileComp = listen<any>("m2m://file-complete", (event) => {
      alert(`File received!\nSaved to: ${event.payload.path}`);
    });

    return () => {
      unlistenMsg.then((f) => f());
      unlistenConn.then((f) => f());
      unlistenFileReq.then((f) => f());
      unlistenFileComp.then((f) => f());
    };
  }, []);

  // Auto-scroll messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleGenerateInvite = async () => {
    try {
      await invoke("start_listening", { address: "127.0.0.1:0" });
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
      setView("chat");
    } catch (e) {
      console.error("Connection failed", e);
      alert("Connection failed: " + e);
    } finally {
      setIsConnecting(false);
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

  const handleSendFile = () => {
    const path = prompt("Enter absolute path to file to send:");
    if (!path || !connection?.peer_key_hex) return;
    invoke("send_file", {
      peerKeyHex: connection.peer_key_hex,
      filePath: path,
    })
      .then(() => {
        setMessages((prev) => [
          ...prev,
          {
            id: Date.now().toString(),
            content: `📎 File request sent: ${path.split(/[\\/]/).pop()}`,
            direction: "sent",
            timestamp: Math.floor(Date.now() / 1000),
          },
        ]);
      })
      .catch((e) => alert("Failed to send file: " + e));
  };

  const acceptFile = async (req: FileRequest) => {
    const dir = prompt("Enter directory path to save to:");
    if (!dir) return;
    try {
      await invoke("accept_file_transfer", {
        peerKeyHex: req.peer_key_hex,
        transferId: req.transfer_id,
        saveDir: dir,
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

  // ═══════════ Hub View ═══════════
  if (view === "hub") {
    return (
      <div className="app-container">
        <div className="header">
          <h1>
            <span>🛡️</span> M2M
          </h1>
          <div className="status-badge">Offline</div>
        </div>

        <div className="content-area centered-view">
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
            </div>

            <div className="section-divider" />

            {/* Fingerprint */}
            <div className="fingerprint-box" id="fingerprint-display">
              <span className="fingerprint-label">Your Identity Fingerprint</span>
              {identity?.fingerprint}
            </div>
          </div>
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
          <div
            className={`status-badge ${
              connection?.state === "established"
                ? "connected"
                : "disconnected"
            }`}
          >
            {connection?.state || "unknown"}
          </div>
          <button
            className="danger"
            onClick={handleDisconnect}
            id="disconnect-btn"
          >
            Disconnect
          </button>
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
              {connection?.peer_fingerprint}
            </span>
          </p>
        </div>

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
