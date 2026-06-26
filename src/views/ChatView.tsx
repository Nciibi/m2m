import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { ToastContainer } from "../toast";
import type {
  Toast,
  ChatMessage,
  ConnectionInfo,
  FileRequest,
  IdentityInfo,
} from "../types";

interface Props {
  connection: ConnectionInfo | null;
  messages: ChatMessage[];
  identity: IdentityInfo | null;
  fileRequests: FileRequest[];
  activeConversationId: string | null;
  toasts: Toast[];
  removeToast: (id: string) => void;
  addToast: (msg: string, type: Toast["type"], duration?: number) => void;

  onSendMessage: (content: string) => Promise<void>;
  onSendFile: () => Promise<void>;
  onVerify: () => Promise<void>;
  onDisconnect: () => Promise<void>;
  onBackToHub: () => void;
  onExportConversation: () => Promise<void>;
  onSetRetention: (policy: string, durationSecs: number | null) => void;

  // Per-conversation retention state
  retentionPolicy: string;
  setRetentionPolicy: (v: string) => void;
  retentionDuration: string;
  setRetentionDuration: (v: string) => void;
}

export default function ChatView({
  connection,
  messages,
  identity,
  fileRequests,
  activeConversationId,
  toasts,
  removeToast,
  addToast,
  onSendMessage,
  onSendFile,
  onVerify,
  onDisconnect,
  onBackToHub,
  onExportConversation,
  onSetRetention,
  retentionPolicy,
  setRetentionPolicy,
  retentionDuration,
  setRetentionDuration,
}: Props) {
  const [inputText, setInputText] = useState("");
  const [showFingerprintModal, setShowFingerprintModal] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to latest message
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!inputText.trim()) return;
    await onSendMessage(inputText);
    setInputText("");
  };

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  };

  return (
    <div className="app-container">
      {/* Header */}
      <div className="header">
        <h1>
          <span
            className="verify-btn"
            onClick={() => setShowFingerprintModal(true)}
            title={
              connection?.peer_verified
                ? "Fingerprint verified"
                : "Verify peer fingerprint"
            }
            style={{ fontSize: "1rem", cursor: "pointer" }}
          >
            {connection?.peer_verified ? "✅" : "⚠️"}
          </span>
          Encrypted Session
        </h1>
        <div className="header-actions">
          <button className="secondary" onClick={onBackToHub}>
            ← Hub
          </button>
          <div
            className={`status-badge ${
              connection?.state === "established" ? "connected" : "disconnected"
            }`}
          >
            {connection?.state || "unknown"}
          </div>
          {connection?.state === "established" && (
            <button
              className="danger"
              onClick={onDisconnect}
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
                  <span
                    style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}
                  >
                    {formatSize(req.total_size)}
                  </span>
                </div>
              </div>
              <div className="file-actions">
                <button
                  onClick={async () => {
                    try {
                      const filePath = await save({
                        title: `Save "${req.filename}" to...`,
                        defaultPath: req.filename,
                      });
                      if (!filePath) return;
                      await invoke("accept_file_transfer", {
                        peerKeyHex: req.peer_key_hex,
                        transferId: req.transfer_id,
                        saveDir: filePath,
                      });
                    } catch (err) {
                      addToast("Accept failed: " + err, "error");
                    }
                  }}
                >
                  Accept
                </button>
                <button
                  className="secondary"
                  onClick={async () => {
                    try {
                      await invoke("reject_file_transfer", {
                        peerKeyHex: req.peer_key_hex,
                        transferId: req.transfer_id,
                      });
                    } catch (err) {
                      addToast("Reject failed: " + err, "error");
                    }
                  }}
                >
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
                  const newPolicy = e.target.value;
                  setRetentionPolicy(newPolicy);
                  const dur =
                    newPolicy === "none"
                      ? null
                      : parseInt(retentionDuration, 10);
                  onSetRetention(newPolicy, dur);
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
                    const newDur = e.target.value;
                    setRetentionDuration(newDur);
                    onSetRetention(retentionPolicy, parseInt(newDur, 10));
                  }}
                >
                  <option value="3600">1 Hour</option>
                  <option value="86400">24 Hours</option>
                  <option value="604800">7 Days</option>
                </select>
              )}
              <button className="secondary" onClick={onExportConversation}>
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
      <form className="input-area" onSubmit={handleSubmit}>
        <button
          type="button"
          className="icon-btn"
          onClick={onSendFile}
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
      <div
        style={{
          padding: "4px 32px 8px",
          display: "flex",
          justifyContent: "space-between",
          fontSize: "0.65rem",
          color: "var(--text-muted)",
        }}
      >
        <span>🔒 End-to-end encrypted</span>
        <span>Ctrl+Enter to send · Esc to go back</span>
      </div>

      {/* Fingerprint Verification Modal */}
      {showFingerprintModal && (
        <div
          className="modal-overlay"
          onClick={() => setShowFingerprintModal(false)}
        >
          <div
            className="modal-content"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <h2>🔐 Verify Peer Fingerprint</h2>
              <button
                className="modal-close"
                onClick={() => setShowFingerprintModal(false)}
              >
                ✕
              </button>
            </div>
            <p
              style={{
                fontSize: "0.85rem",
                color: "var(--text-secondary)",
                marginBottom: "16px",
                lineHeight: 1.5,
              }}
            >
              Compare the fingerprint below with your peer via a secure
              out-of-band channel (in person, phone call, or another verified
              app). Matching fingerprints confirm you're connected to the right
              person.
            </p>
            <div className="fingerprint-comparison">
              <div className="fp-side">
                <h3>You (Local)</h3>
                <div className="fp-grid">
                  {identity?.fingerprint.split(":").map((g, i) => (
                    <span key={i} className="fp-group">
                      {g}
                    </span>
                  ))}
                </div>
              </div>
              <div className="fp-match-row">
                <span className="match-icon">
                  {connection?.peer_verified
                    ? "✅ Matched"
                    : "⬜ Not yet verified"}
                </span>
              </div>
              <div className="fp-side">
                <h3>Peer</h3>
                <div className="fp-grid">
                  {connection?.peer_fingerprint?.split(":").map((g, i) => (
                    <span key={i} className="fp-group">
                      {g}
                    </span>
                  ))}
                </div>
              </div>
            </div>
            {!connection?.peer_verified && (
              <button
                className="verify-modal-btn"
                onClick={async () => {
                  await onVerify();
                  setShowFingerprintModal(false);
                  addToast("Peer fingerprint verified", "success");
                }}
              >
                ✅ Confirm Match & Verify
              </button>
            )}
            {connection?.peer_verified && (
              <p
                style={{
                  textAlign: "center",
                  color: "var(--success)",
                  marginTop: "12px",
                  fontWeight: 600,
                }}
              >
                ✅ Peer verified — fingerprints match
              </p>
            )}
          </div>
        </div>
      )}

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
