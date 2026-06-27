import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { Button, Badge, Modal, ToastContainer } from "../components/ui";
import type {
  Toast as ToastData,
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
  toasts: ToastData[];
  removeToast: (id: string) => void;
  addToast: (msg: string, type: ToastData["type"], duration?: number) => void;

  onSendMessage: (content: string) => Promise<void>;
  onSendFile: () => Promise<void>;
  onVerify: () => Promise<void>;
  onDisconnect: () => Promise<void>;
  onBackToHub: () => void;
  onExportConversation: () => Promise<void>;
  onSetRetention: (policy: string, durationSecs: number | null) => void;

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
  const [scrolledUp, setScrolledUp] = useState(false);
  const [sending, setSending] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);

  // ─── Auto-scroll ───
  useEffect(() => {
    if (!scrolledUp) {
      messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }
    // Reset scrolledUp when new messages arrive from the bottom
    if (!scrolledUp && messagesContainerRef.current) {
      const el = messagesContainerRef.current;
      el.scrollTop = el.scrollHeight;
    }
  }, [messages, scrolledUp]);

  const handleScroll = () => {
    const el = messagesContainerRef.current;
    if (!el) return;
    const isNearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 100;
    setScrolledUp(!isNearBottom);
  };

  // ─── Send ───
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!inputText.trim() || sending) return;
    setSending(true);
    // Trim to max size
    const text = inputText.trim().slice(0, 64 * 1024);
    try {
      await onSendMessage(text);
      setInputText("");
    } finally {
      setSending(false);
    }
  };

  // ─── File send info ───
  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  };

  // ─── Group messages by date ───
  const groupedMessages = groupByDate(messages);

  return (
    <div className="app-container">
      {/* Header */}
      <div className="header">
        <h1>
          <span
            className="verify-btn"
            onClick={() => setShowFingerprintModal(true)}
            title={connection?.peer_verified ? "Fingerprint verified" : "Verify peer fingerprint"}
            style={{ fontSize: "1rem", cursor: "pointer" }}
          >
            {connection?.peer_verified ? "✅" : "⚠️"}
          </span>
          Encrypted Session
        </h1>
        <div className="header-actions">
          <Button variant="secondary" compact onClick={onBackToHub}>
            ← Hub
          </Button>
          <Badge
            variant={connection?.state === "established" ? "success" : "danger"}
            dot
            compact
          >
            {connection?.state || "unknown"}
          </Badge>
          {connection?.state === "established" && (
            <Button variant="danger" compact onClick={onDisconnect} id="disconnect-btn">
              Disconnect
            </Button>
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
                  <span style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)" }}>
                    {formatSize(req.total_size)}
                  </span>
                </div>
              </div>
              <div className="file-actions">
                <Button
                  compact
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
                </Button>
                <Button
                  variant="secondary"
                  compact
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
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Messages */}
      <div
        className="messages"
        id="message-list"
        ref={messagesContainerRef}
        onScroll={handleScroll}
      >
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

        {/* Retention config */}
        {activeConversationId && (
          <div className="retention-config">
            <h4>Conversation Policy</h4>
            <div className="retention-row">
              <select
                value={retentionPolicy}
                onChange={(e) => {
                  const newPolicy = e.target.value;
                  setRetentionPolicy(newPolicy);
                  const dur = newPolicy === "none" ? null : parseInt(retentionDuration, 10);
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
              <Button variant="secondary" compact onClick={onExportConversation}>
                Export Now
              </Button>
            </div>
          </div>
        )}

        {/* Message groups by date */}
        {Object.entries(groupedMessages).map(([dateLabel, msgs]) => (
          <div key={dateLabel}>
            {/* Date separator */}
            <div className="date-separator">
              <span>{dateLabel}</span>
            </div>

            {msgs.map((m) => (
              <div key={m.id} className={`message-bubble ${m.direction}`}>
                {formatMessageContent(m.content)}
                <span className="message-time">
                  {new Date(m.timestamp * 1000).toLocaleTimeString([], {
                    hour: "2-digit",
                    minute: "2-digit",
                  })}
                </span>
              </div>
            ))}
          </div>
        ))}
        <div ref={messagesEndRef} />
      </div>

      {/* Scroll-to-bottom FAB */}
      {scrolledUp && (
        <button
          onClick={() => {
            messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
            setScrolledUp(false);
          }}
          className="scroll-to-bottom-fab"
          aria-label="Scroll to bottom"
        >
          ↓
        </button>
      )}

      {/* Input Area */}
      <form className="input-area" onSubmit={handleSubmit}>
        <Button
          type="button"
          variant="icon"
          onClick={onSendFile}
          id="send-file-btn"
          aria-label="Send a file"
        >
          📎
        </Button>
        <textarea
          id="message-input"
          placeholder="Type a secure message…"
          value={inputText}
          onChange={(e) => {
            // Auto-grow
            const el = e.currentTarget;
            el.style.height = "auto";
            el.style.height = Math.min(el.scrollHeight, 120) + "px";
            setInputText(e.target.value);
          }}
          onKeyDown={(e) => {
            // Ctrl+Enter to send, Shift+Enter for newline
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
              e.preventDefault();
              handleSubmit(e);
            }
            // Esc to clear
            if (e.key === "Escape" && !inputText) {
              onBackToHub();
            }
          }}
          rows={1}
          className="message-input"
          autoFocus
          disabled={connection?.state !== "established"}
        />
        {inputText.length > 64 * 1024 * 0.9 && (
          <span className="char-count-warning">
            {inputText.length}/{64 * 1024}
          </span>
        )}
        <Button
          type="submit"
          className="send-btn"
          id="send-message-btn"
          disabled={!inputText.trim() || sending}
          loading={sending}
        >
          ➤
        </Button>
      </form>

      {/* Footer info */}
      <div
        style={{
          padding: "4px 32px 8px",
          display: "flex",
          justifyContent: "space-between",
          fontSize: "var(--text-xs)",
          color: "var(--color-text-muted)",
        }}
      >
        <span>🔒 End-to-end encrypted</span>
        <span>Ctrl+Enter to send · Esc to go back</span>
      </div>

      {/* Fingerprint Verification Modal */}
      <Modal
        open={showFingerprintModal}
        onClose={() => setShowFingerprintModal(false)}
        title="🔐 Verify Peer Fingerprint"
        footer={
          !connection?.peer_verified ? (
            <Button
              onClick={async () => {
                await onVerify();
                setShowFingerprintModal(false);
                addToast("Peer fingerprint verified", "success");
              }}
            >
              ✅ Confirm Match & Verify
            </Button>
          ) : undefined
        }
      >
        <p style={{ fontSize: "var(--text-md)", lineHeight: 1.5, marginBottom: 16 }}>
          Compare the fingerprint below with your peer via a secure out-of-band channel
          (in person, phone call, or another verified app). Matching fingerprints confirm
          you're connected to the right person.
        </p>

        <div className="fingerprint-comparison">
          <div className="fp-side">
            <h3>You (Local)</h3>
            <div className="fp-grid">
              {identity?.fingerprint.split(":").map((g, i) => (
                <span key={i} className="fp-group">{g}</span>
              ))}
            </div>
          </div>

          <div className="fp-match-row">
            <span className="match-icon">
              {connection?.peer_verified ? "✅ Matched" : "⬜ Not yet verified"}
            </span>
          </div>

          <div className="fp-side">
            <h3>Peer</h3>
            <div className="fp-grid">
              {connection?.peer_fingerprint?.split(":").map((g, i) => (
                <span key={i} className="fp-group">{g}</span>
              ))}
            </div>
          </div>
        </div>

        {connection?.peer_verified && (
          <p style={{ textAlign: "center", color: "var(--color-success)", fontWeight: 600, marginTop: 12 }}>
            ✅ Peer verified — fingerprints match
          </p>
        )}
      </Modal>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}

// ─── Helpers ───

/** Format message content: detect code blocks, emoji, basic formatting */
function formatMessageContent(content: string): React.ReactNode {
  // Simple code block detection (text between backticks)
  const parts = content.split(/(`[^`]+`)/g);
  if (parts.length === 1) return content;

  return parts.map((part, i) => {
    if (part.startsWith("`") && part.endsWith("`")) {
      return (
        <code
          key={i}
          style={{
            background: "rgba(0,0,0,0.3)",
            padding: "2px 6px",
            borderRadius: 4,
            fontFamily: "var(--font-mono)",
            fontSize: "0.85em",
          }}
        >
          {part.slice(1, -1)}
        </code>
      );
    }
    return part;
  });
}

/** Group messages by date label */
function groupByDate(messages: ChatMessage[]): Record<string, ChatMessage[]> {
  const groups: Record<string, ChatMessage[]> = {};
  for (const m of messages) {
    const date = new Date(m.timestamp * 1000);
    const today = new Date();
    const yesterday = new Date(today);
    yesterday.setDate(yesterday.getDate() - 1);

    let label: string;
    if (date.toDateString() === today.toDateString()) {
      label = "Today";
    } else if (date.toDateString() === yesterday.toDateString()) {
      label = "Yesterday";
    } else {
      label = date.toLocaleDateString(undefined, {
        weekday: "long",
        month: "long",
        day: "numeric",
      });
    }

    if (!groups[label]) groups[label] = [];
    groups[label].push(m);
  }
  return groups;
}
