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
    if (!scrolledUp && messagesContainerRef.current) {
      const el = messagesContainerRef.current;
      el.scrollTop = el.scrollHeight;
    }
  }, [messages, scrolledUp]);

  const handleScroll = () => {
    const el = messagesContainerRef.current;
    if (!el) return;
    const isNearBottom =
      el.scrollHeight - el.scrollTop - el.clientHeight < 100;
    setScrolledUp(!isNearBottom);
  };

  // ─── Send ───
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!inputText.trim() || sending) return;
    setSending(true);
    const text = inputText.trim().slice(0, 64 * 1024);
    try {
      await onSendMessage(text);
      setInputText("");
    } finally {
      setSending(false);
    }
  };

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  };

  const groupedMessages = groupByDate(messages);

  return (
    <div className="app-container">
      {/* Premium Header */}
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
            style={{
              fontSize: "1rem",
              cursor: "pointer",
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              width: 28,
              height: 28,
              borderRadius: "var(--radius-sm)",
              background: connection?.peer_verified
                ? "var(--color-success-bg)"
                : "var(--color-warning-bg)",
              transition: "background var(--transition-fast)",
            }}
          >
            {connection?.peer_verified ? "✅" : "⚠️"}
          </span>
          Encrypted Session
        </h1>
        <div className="header-actions">
          <Button variant="secondary" compact onClick={onBackToHub}>
            <span>←</span> Hub
          </Button>
          <Badge
            variant={
              connection?.state === "established" ? "success" : "danger"
            }
            dot
            compact
          >
            {connection?.state || "unknown"}
          </Badge>
          {connection?.state === "established" && (
            <Button
              variant="danger"
              compact
              onClick={onDisconnect}
              id="disconnect-btn"
            >
              Disconnect
            </Button>
          )}
        </div>
      </div>

      {/* File Transfer Requests */}
      {fileRequests.length > 0 && (
        <div
          className="file-requests"
          style={{
            padding: "var(--space-xs) var(--space-xl)",
            display: "flex",
            flexDirection: "column",
            gap: "var(--space-xs)",
            flexShrink: 0,
          }}
        >
          {fileRequests.map((req) => (
            <div
              key={req.transfer_id}
              className="file-request-banner"
              style={{
                background: "var(--color-bg-elevated)",
                border: "1px solid var(--color-border-accent)",
                borderRadius: "var(--radius-md)",
                padding: "var(--space-sm) var(--space-md)",
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                fontSize: "var(--text-sm)",
                animation: "msgSlide 300ms var(--ease-out-expo)",
                boxShadow: "var(--shadow-sm)",
              }}
            >
              <div
                className="file-info"
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "var(--space-sm)",
                }}
              >
                <div
                  style={{
                    width: 36,
                    height: 36,
                    borderRadius: "var(--radius-sm)",
                    background: "var(--color-accent-glow-subtle)",
                    border: "1px solid var(--color-border-accent)",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    fontSize: "1rem",
                    flexShrink: 0,
                  }}
                >
                  📄
                </div>
                <div>
                  <strong
                    style={{ fontSize: "var(--text-base)", color: "var(--color-text-primary)" }}
                  >
                    {req.filename}
                  </strong>
                  <br />
                  <span
                    style={{
                      fontSize: "var(--text-xs)",
                      color: "var(--color-text-muted)",
                    }}
                  >
                    {formatSize(req.total_size)}
                  </span>
                </div>
              </div>
              <div
                className="file-actions"
                style={{ display: "flex", gap: "var(--space-xs)" }}
              >
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
        style={{
          flex: 1,
          overflowY: "auto",
          padding: "var(--space-xl) var(--space-2xl)",
          display: "flex",
          flexDirection: "column",
          gap: "var(--space-sm)",
          scrollbarWidth: "thin",
          scrollbarColor: "var(--scrollbar-thumb) transparent",
        }}
      >
        <div
          className="session-banner"
          style={{
            textAlign: "center",
            padding: "var(--space-xl)",
            marginBottom: "var(--space-sm)",
          }}
        >
          <div
            className="lock-icon"
            style={{
              display: "inline-flex",
              width: 48,
              height: 48,
              borderRadius: "50%",
              background: "var(--color-accent-glow-subtle)",
              border: "1px solid var(--color-border-accent)",
              alignItems: "center",
              justifyContent: "center",
              fontSize: "1.2rem",
              marginBottom: "var(--space-sm)",
              boxShadow: "0 0 20px var(--color-accent-glow)",
            }}
          >
            🔒
          </div>
          <p
            style={{
              color: "var(--color-text-muted)",
              fontSize: "var(--text-sm)",
              lineHeight: 1.6,
              margin: 0,
            }}
          >
            End-to-end encrypted session established.
            <br />
            <span
              className="peer-fp"
              style={{
                fontFamily: "var(--font-mono)",
                color: "var(--color-text-secondary)",
                fontSize: "var(--text-sm)",
                display: "inline-block",
                marginTop: "var(--space-xs)",
                background: "rgba(0,0,0,0.25)",
                padding: "var(--space-xxs) var(--space-sm)",
                borderRadius: "var(--radius-sm)",
                border: "1px solid var(--color-border-default)",
              }}
            >
              {connection?.peer_fingerprint || activeConversationId}
            </span>
          </p>
        </div>

        {/* Retention config */}
        {activeConversationId && (
          <div
            className="retention-config"
            style={{
              padding: "var(--space-md) var(--space-lg)",
              background: "rgba(0,0,0,0.2)",
              border: "1px solid var(--color-border-default)",
              borderRadius: "var(--radius-lg)",
              marginBottom: "var(--space-md)",
            }}
          >
            <h4
              style={{
                fontSize: "var(--text-sm)",
                fontWeight: 600,
                marginBottom: "var(--space-sm)",
                color: "var(--color-text-primary)",
                display: "flex",
                alignItems: "center",
                gap: "var(--space-xs)",
              }}
            >
              <span>⚙️</span> Conversation Policy
            </h4>
            <div style={{ display: "flex", gap: "var(--space-sm)", alignItems: "center", flexWrap: "wrap" }}>
              <select
                value={retentionPolicy}
                onChange={(e) => {
                  const newPolicy = e.target.value;
                  setRetentionPolicy(newPolicy);
                  const dur = newPolicy === "none" ? null : parseInt(retentionDuration, 10);
                  onSetRetention(newPolicy, dur);
                }}
                style={{
                  background: "var(--color-bg-input)",
                  border: "1px solid var(--color-border-default)",
                  color: "var(--color-text-primary)",
                  padding: "8px 14px",
                  borderRadius: "var(--radius-md)",
                  fontSize: "var(--text-sm)",
                  fontFamily: "inherit",
                  outline: "none",
                  cursor: "pointer",
                  transition: "var(--transition-fast)",
                  appearance: "none",
                  paddingRight: 36,
                  backgroundImage: `url("data:image/svg+xml;charset=UTF-8,%3csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='rgba(255,255,255,0.5)' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3e%3cpolyline points='6 9 12 15 18 9'%3e%3c/polyline%3e%3c/svg%3e")`,
                  backgroundRepeat: "no-repeat",
                  backgroundPosition: "right 12px center",
                  backgroundSize: 14,
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
                  style={{
                    background: "var(--color-bg-input)",
                    border: "1px solid var(--color-border-default)",
                    color: "var(--color-text-primary)",
                    padding: "8px 14px",
                    borderRadius: "var(--radius-md)",
                    fontSize: "var(--text-sm)",
                    fontFamily: "inherit",
                    outline: "none",
                    cursor: "pointer",
                    transition: "var(--transition-fast)",
                    appearance: "none",
                    paddingRight: 36,
                    backgroundImage: `url("data:image/svg+xml;charset=UTF-8,%3csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='rgba(255,255,255,0.5)' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3e%3cpolyline points='6 9 12 15 18 9'%3e%3c/polyline%3e%3c/svg%3e")`,
                    backgroundRepeat: "no-repeat",
                    backgroundPosition: "right 12px center",
                    backgroundSize: 14,
                  }}
                >
                  <option value="3600">1 Hour</option>
                  <option value="86400">24 Hours</option>
                  <option value="604800">7 Days</option>
                </select>
              )}
              <Button
                variant="secondary"
                compact
                onClick={onExportConversation}
              >
                Export Now
              </Button>
            </div>
          </div>
        )}

        {/* Message groups by date */}
        {Object.entries(groupedMessages).map(([dateLabel, msgs]) => (
          <div key={dateLabel}>
            {/* Date separator */}
            <div
              className="date-separator"
              style={{
                display: "flex",
                alignItems: "center",
                gap: "var(--space-sm)",
                margin: "var(--space-lg) 0 var(--space-sm)",
                padding: "0 var(--space-xs)",
              }}
            >
              <span
                style={{
                  flex: 1,
                  height: 1,
                  background: "var(--color-border-default)",
                }}
              />
              <span
                style={{
                  fontSize: "var(--text-xs)",
                  color: "var(--color-text-muted)",
                  fontWeight: 500,
                  whiteSpace: "nowrap",
                  textTransform: "uppercase",
                  letterSpacing: "0.08em",
                }}
              >
                {dateLabel}
              </span>
              <span
                style={{
                  flex: 1,
                  height: 1,
                  background: "var(--color-border-default)",
                }}
              />
            </div>

            {msgs.map((m, idx) => (
              <div
                key={m.id}
                className={`message-bubble ${m.direction}`}
                style={{
                  maxWidth: "75%",
                  padding: "var(--space-sm) var(--space-md)",
                  borderRadius: "var(--radius-lg)",
                  fontSize: "var(--text-md)",
                  lineHeight: 1.6,
                  animation: `msgSlide 0.4s var(--ease-out-expo) ${idx * 0.05}s both`,
                  position: "relative",
                  wordWrap: "break-word",
                  alignSelf:
                    m.direction === "sent" ? "flex-end" : "flex-start",
                  background:
                    m.direction === "sent"
                      ? "var(--color-accent-gradient)"
                      : "var(--color-bg-elevated)",
                  color: "white",
                  borderBottomRightRadius: m.direction === "sent" ? 4 : "var(--radius-lg)",
                  borderBottomLeftRadius: m.direction === "received" ? 4 : "var(--radius-lg)",
                  boxShadow:
                    m.direction === "sent"
                      ? "var(--shadow-bubble-sent)"
                      : "var(--shadow-bubble-received)",
                  border: m.direction === "received"
                    ? "1px solid rgba(255,255,255,0.04)"
                    : "1px solid rgba(255,255,255,0.1)",
                }}
              >
                {formatMessageContent(m.content)}
                <span
                  className="message-time"
                  style={{
                    fontSize: "var(--text-xs)",
                    opacity: m.direction === "sent" ? 0.7 : 0.5,
                    marginTop: "var(--space-xxs)",
                    textAlign: "right",
                    display: "block",
                    letterSpacing: "0.02em",
                    color: m.direction === "sent" ? "white" : undefined,
                  }}
                >
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
          style={{
            position: "absolute",
            bottom: 80,
            right: 32,
            zIndex: "var(--z-base)",
            width: 40,
            height: 40,
            borderRadius: "50%",
            background: "var(--color-accent-gradient)",
            color: "white",
            border: "1px solid rgba(255,255,255,0.15)",
            fontSize: "1.2rem",
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            boxShadow: "var(--shadow-accent-strong)",
            animation: "fabAppear 0.3s var(--ease-out-expo)",
            transition: "var(--transition-fast)",
            fontFamily: "inherit",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.transform = "translateY(-2px)";
            e.currentTarget.style.boxShadow = "0 8px 24px var(--color-accent-glow-strong)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.transform = "";
            e.currentTarget.style.boxShadow = "var(--shadow-accent-strong)";
          }}
        >
          ↓
        </button>
      )}

      {/* Input Area */}
      <form
        className="input-area"
        onSubmit={handleSubmit}
        style={{
          padding: "var(--space-md) var(--space-2xl)",
          borderTop: "1px solid var(--color-border-default)",
          display: "flex",
          gap: "var(--space-sm)",
          alignItems: "flex-end",
          background: "linear-gradient(to top, rgba(0,0,0,0.25), transparent)",
          position: "relative",
          flexShrink: 0,
        }}
      >
        <button
          type="button"
          onClick={onSendFile}
          className="icon-btn"
          id="send-file-btn"
          aria-label="Send a file"
          title="Send file"
          style={{ flexShrink: 0, marginBottom: 2 }}
        >
          📎
        </button>

        <div style={{ flex: 1, position: "relative", minWidth: 0 }}>
          <textarea
            id="message-input"
            placeholder="Type a secure message…"
            value={inputText}
            onChange={(e) => {
              const el = e.currentTarget;
              el.style.height = "auto";
              el.style.height = Math.min(el.scrollHeight, 120) + "px";
              setInputText(e.target.value);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                handleSubmit(e);
              }
              if (e.key === "Escape" && !inputText) {
                onBackToHub();
              }
            }}
            rows={1}
            disabled={connection?.state !== "established"}
            style={{
              width: "100%",
              fontSize: "var(--text-md)",
              padding: "12px 18px",
              borderRadius: "var(--radius-xl)",
              background: "var(--color-bg-input)",
              border: "1px solid var(--color-border-default)",
              color: "var(--color-text-primary)",
              fontFamily: "inherit",
              outline: "none",
              resize: "none",
              lineHeight: 1.5,
              maxHeight: 120,
              transition: "var(--transition-fast)",
              boxShadow: "var(--shadow-inner)",
            }}
            onFocus={(e) => {
              e.currentTarget.style.borderColor =
                "var(--color-border-active)";
              e.currentTarget.style.boxShadow =
                "0 0 0 3px var(--color-accent-glow)";
              e.currentTarget.style.background =
                "var(--color-bg-input-focus)";
            }}
            onBlur={(e) => {
              e.currentTarget.style.borderColor =
                "var(--color-border-default)";
              e.currentTarget.style.boxShadow = "var(--shadow-inner)";
              e.currentTarget.style.background = "var(--color-bg-input)";
            }}
          />
          {inputText.length > 64 * 1024 * 0.9 && (
            <span
              className="char-count-warning"
              style={{
                position: "absolute",
                right: 12,
                bottom: 6,
                fontSize: "var(--text-xs)",
                color: "var(--color-warning)",
              }}
            >
              {inputText.length}/{64 * 1024}
            </span>
          )}
        </div>

        <button
          type="submit"
          id="send-message-btn"
          disabled={!inputText.trim() || sending || connection?.state !== "established"}
          style={{
            background: !inputText.trim() || sending
              ? "var(--color-bg-input)"
              : "var(--color-accent-gradient)",
            color: !inputText.trim() ? "var(--color-text-muted)" : "white",
            border: "1px solid var(--color-border-default)",
            padding: "10px 16px",
            minWidth: 46,
            minHeight: 42,
            borderRadius: "var(--radius-lg)",
            cursor: !inputText.trim() || connection?.state !== "established" ? "not-allowed" : "pointer",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: "1.2rem",
            fontFamily: "inherit",
            transition: "var(--transition-base)",
            boxShadow: !inputText.trim() ? "none" : "var(--shadow-accent)",
            opacity: !inputText.trim() ? 0.5 : 1,
            flexShrink: 0,
            marginBottom: 2,
          }}
          onMouseEnter={(e) => {
            if (inputText.trim() && !sending) {
              e.currentTarget.style.transform = "translateY(-1px)";
              e.currentTarget.style.boxShadow =
                "var(--shadow-accent-strong)";
            }
          }}
          onMouseLeave={(e) => {
            if (inputText.trim()) {
              e.currentTarget.style.transform = "";
              e.currentTarget.style.boxShadow = "var(--shadow-accent)";
            }
          }}
        >
          {sending ? (
            <span
              style={{
                width: 18,
                height: 18,
                border: "2px solid rgba(255,255,255,0.3)",
                borderTopColor: "white",
                borderRadius: "50%",
                display: "inline-block",
                animation: "spin 0.6s linear infinite",
              }}
            />
          ) : (
            "➤"
          )}
        </button>
      </form>

      {/* Footer */}
      <div
        style={{
          padding: "4px var(--space-2xl) 8px",
          display: "flex",
          justifyContent: "space-between",
          fontSize: "var(--text-xs)",
          color: "var(--color-text-muted)",
          background: "rgba(0,0,0,0.15)",
          flexShrink: 0,
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
        <p
          style={{
            fontSize: "var(--text-md)",
            lineHeight: 1.5,
            marginBottom: "var(--space-md)",
            color: "var(--color-text-secondary)",
          }}
        >
          Compare the fingerprint below with your peer via a secure
          out-of-band channel (in person, phone call, or another verified
          app). Matching fingerprints confirm you're connected to the right
          person.
        </p>

        <div
          className="fingerprint-comparison"
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "var(--space-lg)",
          }}
        >
          <div
            className="fp-side"
            style={{
              background: "rgba(0,0,0,0.2)",
              borderRadius: "var(--radius-md)",
              padding: "var(--space-md)",
              border: "1px solid var(--color-border-default)",
            }}
          >
            <h3
              style={{
                fontSize: "var(--text-xs)",
                textTransform: "uppercase",
                letterSpacing: "0.1em",
                color: "var(--color-text-muted)",
                marginBottom: "var(--space-sm)",
                fontWeight: 600,
              }}
            >
              You (Local)
            </h3>
            <div className="fp-grid" style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 6 }}>
              {identity?.fingerprint.split(":").map((g, i) => (
                <span
                  key={i}
                  className="fp-group"
                  style={{
                    fontFamily: "var(--font-mono)",
                    fontSize: "var(--text-sm)",
                    background: "rgba(0,0,0,0.2)",
                    padding: "6px 4px",
                    borderRadius: 4,
                    textAlign: "center",
                    color: "var(--color-text-accent)",
                    letterSpacing: "0.5px",
                    border: "1px solid var(--color-border-default)",
                  }}
                >
                  {g}
                </span>
              ))}
            </div>
          </div>

          <div
            className="fp-match-row"
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "var(--space-xs)",
              fontSize: "var(--text-sm)",
              color: "var(--color-text-muted)",
            }}
          >
            <span
              className="match-icon"
              style={{
                color: connection?.peer_verified
                  ? "var(--color-success)"
                  : "var(--color-text-muted)",
              }}
            >
              {connection?.peer_verified ? "✅ Matched" : "⬜ Not yet verified"}
            </span>
          </div>

          <div
            className="fp-side"
            style={{
              background: "rgba(0,0,0,0.2)",
              borderRadius: "var(--radius-md)",
              padding: "var(--space-md)",
              border: "1px solid var(--color-border-default)",
            }}
          >
            <h3
              style={{
                fontSize: "var(--text-xs)",
                textTransform: "uppercase",
                letterSpacing: "0.1em",
                color: "var(--color-text-muted)",
                marginBottom: "var(--space-sm)",
                fontWeight: 600,
              }}
            >
              Peer
            </h3>
            <div className="fp-grid" style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 6 }}>
              {connection?.peer_fingerprint?.split(":").map((g, i) => (
                <span
                  key={i}
                  className="fp-group"
                  style={{
                    fontFamily: "var(--font-mono)",
                    fontSize: "var(--text-sm)",
                    background: "rgba(0,0,0,0.2)",
                    padding: "6px 4px",
                    borderRadius: 4,
                    textAlign: "center",
                    color: "var(--color-text-accent)",
                    letterSpacing: "0.5px",
                    border: "1px solid var(--color-border-default)",
                  }}
                >
                  {g}
                </span>
              ))}
            </div>
          </div>
        </div>

        {connection?.peer_verified && (
          <p
            style={{
              textAlign: "center",
              color: "var(--color-success)",
              fontWeight: 600,
              marginTop: "var(--space-md)",
            }}
          >
            ✅ Peer verified — fingerprints match
          </p>
        )}
      </Modal>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}

// ─── Helpers ───

function formatMessageContent(content: string): React.ReactNode {
  const parts = content.split(/(`[^`]+`)/g);
  if (parts.length === 1) return content;

  return parts.map((part, i) => {
    if (part.startsWith("`") && part.endsWith("`")) {
      return (
        <code
          key={i}
          style={{
            background: "rgba(0,0,0,0.25)",
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
