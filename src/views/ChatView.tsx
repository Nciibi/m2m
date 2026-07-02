import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { Button, Badge, Modal, ToastContainer, ProgressBar } from "../components/ui";
import {
  ArrowLeftIcon, ShieldIcon, VerifiedIcon, LockIcon,
  SendIcon, AttachIcon, FileIcon, ArrowDownIcon, CloseIcon,
  SmileyIcon, CheckDoubleIcon, ClockIcon,
} from "../components/ui/Icons";
import { useApp } from "../context/AppContext";
import { useChat } from "../context/ChatContext";
import type { ChatMessage } from "../types";

export default function ChatView() {
  // Typing peers state (ids of peers currently typing)
  const [typingPeers, setTypingPeers] = useState<string[]>([]);
  const { identity, toasts, removeToast, addToast, setView } = useApp();
  const {
    connection, messages, fileRequests, activeConversationId,
    reconnecting, reconnectAttempt,
    handleSendMessage, handleSendMessageWithTimer, handleSendFile, handleVerify, handleDisconnect,
    handleReconnect,
    handleExportConversation, handleSetRetention,
    retentionPolicy, setRetentionPolicy, retentionDuration, setRetentionDuration,
    handleSendReaction, handleRemoveReaction, handleMarkConversationRead,
    handleEditMessage, handleDeleteMessage,
  } = useChat();
  const [text, setText] = useState("");
  const [showFp, setShowFp] = useState(false);
  const [scrolledUp, setScrolledUp] = useState(false);
  const [sending, setSending] = useState(false);
  const [pickerMsgId, setPickerMsgId] = useState<string | null>(null);
  const [contextMsgId, setContextMsgId] = useState<string | null>(null);
  const [editingMsgId, setEditingMsgId] = useState<string | null>(null);
  const [editText, setEditText] = useState("");
  const [timerSecs, setTimerSecs] = useState<number>(0);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [hasOlder, setHasOlder] = useState(true);
  const [pageLoadKey, setPageLoadKey] = useState(0);
  // Message status tracking: messageId → "sending" | "sent" | "delivered" | "read"
  const [msgStatus, setMsgStatus] = useState<Record<string, "sending" | "sent" | "delivered" | "read">>({});
  // File transfer progress: transferId → TransferProgress
  const [fileProgress, setFileProgress] = useState<Record<string, { filename: string; total_size: number; bytes_transferred: number; chunks_completed: number; chunks_total: number; speed_bytes_per_sec: number; estimated_remaining_secs: number; state: string; transfer_id: string }>>({});
  // Emoji picker state
  const [emojiPickerOpen, setEmojiPickerOpen] = useState(false);
  const emojiBtnRef = useRef<HTMLButtonElement>(null);
  const endRef = useRef<HTMLDivElement>(null);
  const msgRef = useRef<HTMLDivElement>(null);

  useEffect(() => { if (!scrolledUp && msgRef.current) msgRef.current.scrollTop = msgRef.current.scrollHeight; }, [messages, scrolledUp]);

  // Periodic cleanup of expired self-destruct messages
  useEffect(() => {
    const timer = setInterval(() => {
      const now = Math.floor(Date.now() / 1000);
      setMessages((prev: ChatMessage[]) => prev.filter((m) => !m.expires_at || m.expires_at > now));
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  // Also call backend cleanup periodically
  useEffect(() => {
    const timer = setInterval(() => {
      invoke("cleanup_expired_messages").catch(() => {});
    }, 60000);
    return () => clearInterval(timer);
  }, []);

  // Mark messages as read when viewing the chat
  useEffect(() => {
    const hasUnreadReceived = messages.some((m) => m.direction === "received" && m.read_at === null);
    if (hasUnreadReceived && activeConversationId) {
      const timer = setTimeout(() => handleMarkConversationRead(), 1000);
      return () => clearTimeout(timer);
    }
  }, [messages, activeConversationId, handleMarkConversationRead]);

  // Load older messages when user scrolls to top
  const onScroll = useCallback(async () => {
    const el = msgRef.current;
    if (!el) return;
    const atTop = el.scrollTop <= 50;
    setScrolledUp(el.scrollHeight - el.scrollTop - el.clientHeight > 100);

    if (atTop && hasOlder && !loadingOlder && messages.length > 0 && activeConversationId) {
      setLoadingOlder(true);
      const oldestTimestamp = messages.reduce((minT, m) => Math.min(minT, m.timestamp), Infinity);
      try {
        const older = await invoke<ChatMessage[]>("load_messages", {
          peerKeyHex: activeConversationId,
          beforeTimestamp: oldestTimestamp,
          limit: 100,
        });
        if (older.length === 0) {
          setHasOlder(false);
        } else {
          // Preserve scroll position by tracking height before prepend
          const prevHeight = el.scrollHeight;
          setMessages((prev: ChatMessage[]) => {
            // Deduplicate by ID in case of overlap
            const existingIds = new Set(prev.map(m => m.id));
            const newMsgs = older.filter(m => !existingIds.has(m.id));
            return [...newMsgs, ...prev];
          });
          // On next frame, adjust scroll to keep position after prepend
          requestAnimationFrame(() => {
            const newHeight = el.scrollHeight;
            el.scrollTop = newHeight - prevHeight;
          });
          setPageLoadKey(k => k + 1);
        }
      } catch { /* noop — older messages may not exist */ }
      setLoadingOlder(false);
    }
  }, [messages, hasOlder, loadingOlder, activeConversationId]);

  // Listen for file transfer progress events
  useEffect(() => {
    const unlisten = listen<any>("m2m://transfer-progress", (event) => {
      setFileProgress((prev) => ({ ...prev, [event.payload.transfer_id]: event.payload }));
    });
    const unlistenComplete = listen<any>("m2m://file-complete", (event) => {
      setFileProgress((prev) => {
        const next = { ...prev };
        if (event.payload.transfer_id && next[event.payload.transfer_id]) {
          next[event.payload.transfer_id] = { ...next[event.payload.transfer_id], state: "completed" };
        }
        return next;
      });
    });
    const unlistenCancelled = listen<any>("m2m://transfer-cancelled", (event) => {
      setFileProgress((prev) => {
        const next = { ...prev };
        if (event.payload.transfer_id && next[event.payload.transfer_id]) {
          next[event.payload.transfer_id] = { ...next[event.payload.transfer_id], state: "cancelled" };
        }
        return next;
      });
    });
    return () => {
      unlisten.then(f => f());
      unlistenComplete.then(f => f());
      unlistenCancelled.then(f => f());
    };
  }, []);

  // Emoji list for the picker
  const EMOJIS = ["😀","😁","😂","🤣","😊","😉","😍","🥰","😘","😜","😎","🤩",
    "👍","👎","✌️","🤞","👊","💪","🙌","👏","🤝","🔥","⭐","💯",
    "❤️","🧡","💛","💚","💙","💜","🖤","🤍","💔","💖","✨","🎉",
    "🙏","💀","☠️","👋","🫂","🤗","😤","😭","😱","🤔","🙄","😴",
    "✅","❌","❗","❓","➕","➖","🚀","🎂","🎁","💰","🔒","🔓",
  ];

  // Close emoji picker on click outside
  useEffect(() => {
    if (!emojiPickerOpen) return;
    const handler = (e: MouseEvent) => {
      if (emojiBtnRef.current && !emojiBtnRef.current.contains(e.target as Node)) {
        setEmojiPickerOpen(false);
      }
    };
    window.addEventListener("click", handler);
    return () => window.removeEventListener("click", handler);
  }, [emojiPickerOpen]);

  // Reset pagination state when conversation changes
  useEffect(() => {
    setHasOlder(true);
    setLoadingOlder(false);
  }, [activeConversationId]);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!text.trim() || sending) return;
    setSending(true);
    const content = text.trim().slice(0, 64 * 1024);
    const tempId = `temp-${Date.now()}`;
    // Show "Sending…" status immediately
    setMsgStatus((prev) => ({ ...prev, [tempId]: "sending" }));
    try {
      if (timerSecs > 0) {
        await handleSendMessageWithTimer(content, timerSecs);
      } else {
        await handleSendMessage(content);
      }
      setText("");
      setTimerSecs(0);
      // Mark as "sent" — the real message will replace the temp ID
      setMsgStatus((prev) => {
        const next = { ...prev };
        delete next[tempId];
        return next;
      });
    } finally { setSending(false); }
  };

  // Close context menu on click outside
  useEffect(() => {
    if (!contextMsgId) return;
    const handler = () => setContextMsgId(null);
    window.addEventListener("click", handler, { once: true });
    return () => window.removeEventListener("click", handler);
  }, [contextMsgId]);

  const fmt = (b: number) => b < 1024 ? `${b} B` : b < 1048576 ? `${(b / 1024).toFixed(1)} KB` : `${(b / 1048576).toFixed(1)} MB`;

  const grouped = groupByDate(messages);

  const backToHub = () => setView("hub");

  return (
    <div className="app-shell">
      <div className="app-header">
        <h1 className="app-header__title">
          <span onClick={() => setShowFp(true)} title={connection?.peer_verified ? "Verified" : "Verify"}
            className={`app-header__icon-bg ${connection?.peer_verified ? 'app-header__icon-bg--success' : 'app-header__icon-bg--warning'}`}>
            {connection?.peer_verified ? <VerifiedIcon size={16} color="var(--color-success)" /> : <ShieldIcon size={16} color="var(--color-warning)" />}
          </span>
          Encrypted Session
        </h1>
        <div className="app-header__actions">
          <Button variant="secondary" size="sm" onClick={backToHub}><ArrowLeftIcon size={16} /> Hub</Button>
          {connection?.state === "disconnected" && connection?.peer_verified ? (
            <Button variant="warning" size="sm" onClick={handleReconnect} disabled={reconnecting}>
              {reconnecting ? `Reconnecting (${reconnectAttempt}/5)…` : "Reconnect"}
            </Button>
          ) : (
            <Badge variant={connection?.state === "established" ? "success" : "danger"} dot compact>
              {reconnecting ? `Reconnecting…` : (connection?.state || "unknown")}
            </Badge>
          )}
          {connection?.state === "established" && <Button variant="danger" size="sm" onClick={handleDisconnect} id="disconnect-btn">Disconnect</Button>}
        </div>
      </div>

      {/* File requests */}
      {fileRequests.length > 0 && (
        <div className="file-req-area">
          {fileRequests.map(r => (
            <div key={r.transfer_id} className="file-req">
              <div className="file-req__info">
                <div className="file-req__icon"><FileIcon size={18} color="var(--color-accent-bright)" /></div>
                <div><div className="file-req__name">{r.filename}</div><span className="file-req__size">{fmt(r.total_size)}</span></div>
              </div>
              <div className="file-req__actions">
                <Button size="xs" onClick={async () => {
                  const p = await save({ title: `Save "${r.filename}"`, defaultPath: r.filename });
                  if (p) invoke("accept_file_transfer", { peerKeyHex: r.peer_key_hex, transferId: r.transfer_id, saveDir: p }).catch(e => addToast("Accept failed: " + e, "error"));
                }}>Accept</Button>
                <Button variant="secondary" size="xs" onClick={() => invoke("reject_file_transfer", { peerKeyHex: r.peer_key_hex, transferId: r.transfer_id }).catch(e => addToast("Reject failed: " + e, "error"))}>Reject</Button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Messages */}
      <div className="msg-area" ref={msgRef} onScroll={onScroll} id="message-list">
        {loadingOlder && <div className="msg-loading-older">Loading older messages…</div>}
        {!hasOlder && messages.length > 0 && <div className="msg-loading-older">Beginning of conversation</div>}
        <div className="session-banner">
          <div className="session-banner__icon"><LockIcon size={22} color="var(--color-accent-bright)" /></div>
          <p className="session-banner__text">
            End-to-end encrypted session established.<br />
            <span className="session-banner__fp">{connection?.peer_fingerprint || activeConversationId}</span>
          </p>
        </div>

        {activeConversationId && (
          <div className="retention-config">
            <div className="retention-config__title">Conversation Policy</div>
            <div className="retention-row">
              <div className="select-wrap" style={{ width: 'auto' }}>
                <select className="select--compact" value={retentionPolicy} onChange={e => { setRetentionPolicy(e.target.value); handleSetRetention(e.target.value, e.target.value === "none" ? null : parseInt(retentionDuration, 10)); }}>
                  <option value="none">No Expiration</option>
                  <option value="delete">Auto-Delete After</option>
                  <option value="export">Auto-Export After</option>
                </select>
              </div>
              {retentionPolicy !== "none" && (
                <div className="select-wrap" style={{ width: 'auto' }}>
                  <select className="select--compact" value={retentionDuration} onChange={e => { setRetentionDuration(e.target.value); handleSetRetention(retentionPolicy, parseInt(e.target.value, 10)); }}>
                    <option value="3600">1 Hour</option>
                    <option value="86400">24 Hours</option>
                    <option value="604800">7 Days</option>
                  </select>
                </div>
              )}
              <Button variant="secondary" size="xs" onClick={handleExportConversation}>Export Now</Button>
            </div>
          </div>
        )}

        {Object.entries(grouped).map(([label, msgs]: [string, any]) => (
          <div key={label}>
            <div className="date-sep">
              <span className="date-sep__line" />
              <span className="date-sep__label">{label}</span>
              <span className="date-sep__line" />
            </div>
            {msgs.map((m: ChatMessage, i: number) => (
              <div key={m.id} className={`msg-bubble msg-bubble--${m.direction}${m.deleted ? ' msg-bubble--deleted' : ''}`}
                style={{ animationDelay: `${i * 0.05}s` }}
                onMouseEnter={() => setPickerMsgId(m.id)}
                onMouseLeave={() => setPickerMsgId(null)}
                onContextMenu={(e) => { e.preventDefault(); setContextMsgId(m.id); }}
              >
                {m.deleted ? (
                  <em style={{ opacity: 0.5, fontStyle: 'italic' }}>Message deleted</em>
                ) : editingMsgId === m.id ? (
                  /* Inline edit mode */
                  <div className="msg-edit-inline">
                    <textarea className="msg-edit-input" value={editText}
                      onChange={e => setEditText(e.target.value)}
                      onKeyDown={async (e) => {
                        if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                          e.preventDefault();
                          await handleEditMessage(m.id, editText);
                          setEditingMsgId(null);
                        }
                        if (e.key === "Escape") setEditingMsgId(null);
                      }}
                      autoFocus
                      rows={2}
                    />
                    <div className="msg-edit-actions">
                      <Button size="xs" onClick={async () => { await handleEditMessage(m.id, editText); setEditingMsgId(null); }}>Save</Button>
                      <Button variant="secondary" size="xs" onClick={() => setEditingMsgId(null)}>Cancel</Button>
                    </div>
                  </div>
                ) : (
                  /* Normal message rendering with markdown */
                  <div className="msg-content">{renderMarkdown(m.content)}</div>
                )}
                <span className="msg-footer-row">
                  <span className="msg-time">{new Date(m.timestamp * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
                  {/* Edited badge */}
                  {m.edited_at !== null && !m.deleted && (
                    <span className="msg-edited-badge" title={`Edited ${new Date(m.edited_at * 1000).toLocaleString()}`}>edited</span>
                  )}
                  {/* Self-destruct timer */}
                  {m.expires_at !== null && !m.deleted && !m.direction.startsWith("sent") && (
                    <SelfDestructTimer expiresAt={m.expires_at} />
                  )}
                  {/* Read receipt for received messages */}
                  {m.direction === "received" && m.read_at !== null && (
                    <span className="msg-read-badge" title={`Read ${new Date(m.read_at * 1000).toLocaleString()}`}>
                      ✓✓
                    </span>
                  )}
                </span>
                {/* Reactions */}
                {Object.keys(m.reactions || {}).length > 0 && !m.deleted && (
                  <div className="msg-reactions">
                    {Object.entries(m.reactions).map(([emoji, reactors]) => (
                      <button
                        key={emoji}
                        className={`msg-reaction ${reactors.includes("self") ? "msg-reaction--self" : ""}`}
                        onClick={() => {
                          if (reactors.includes("self")) {
                            handleRemoveReaction(m.id, emoji);
                          } else {
                            handleSendReaction(m.id, emoji);
                          }
                        }}
                        title={reactors.join(", ")}
                      >
                        {emoji} {reactors.length}
                      </button>
                    ))}
                  </div>
                )}
                {/* Reaction picker on hover */}
                {pickerMsgId === m.id && !m.deleted && (
                  <div className="reaction-picker">
                    {["👍", "❤️", "😂", "😮", "😢", "🙏"].map((emoji) => (
                      <button
                        key={emoji}
                        className={`reaction-picker__btn ${(m.reactions?.[emoji] || []).includes("self") ? "reaction-picker__btn--active" : ""}`}
                        onClick={(e) => {
                          e.stopPropagation();
                          const reactors = m.reactions?.[emoji] || [];
                          if (reactors.includes("self")) {
                            handleRemoveReaction(m.id, emoji);
                          } else {
                            handleSendReaction(m.id, emoji);
                          }
                          setPickerMsgId(null);
                        }}
                      >
                        {emoji}
                      </button>
                    ))}
                  </div>
                )}
                {/* Context menu */}
                {contextMsgId === m.id && !m.deleted && (
                  <div className="msg-context-menu" onClick={(e) => e.stopPropagation()}>
                    <button className="msg-context-item" onClick={() => { setEditingMsgId(m.id); setEditText(m.content); setContextMsgId(null); }}>
                      Edit
                    </button>
                    <button className="msg-context-item msg-context-item--danger" onClick={async () => { await handleDeleteMessage(m.id); setContextMsgId(null); }}>
                      Delete
                    </button>
                  </div>
                )}
              </div>
            ))}
          </div>
        ))}

        {messages.length === 0 && (
          <div className="conv-empty" style={{ marginTop: 'var(--space-2xl)' }}>
            <SendIcon size={48} color="var(--color-text-muted)" />
            <span style={{ fontSize: 'var(--text-lg)', fontWeight: 600, color: 'var(--color-text-primary)' }}>
              Start the conversation
            </span>
            <span style={{ maxWidth: '320px', textAlign: 'center', lineHeight: 1.6 }}>
              Send a message below to begin your encrypted conversation. All messages are protected with end-to-end encryption.
            </span>
          </div>
        )}

        <div ref={endRef} />
      </div>

      {/* FAB */}
      {scrolledUp && (
        <button className="scroll-fab" onClick={() => { endRef.current?.scrollIntoView({ behavior: "smooth" }); setScrolledUp(false); }}
          aria-label="Scroll to bottom">
          <ArrowDownIcon size={20} />
        </button>
      )}

      {/* Input */}
      <form className="msg-input-area" onSubmit={submit}>
        <button type="button" className="msg-attach-btn" onClick={handleSendFile} id="send-file-btn" aria-label="Send file"><AttachIcon size={20} /></button>
        <div className="msg-input-wrap">
          <textarea id="message-input" placeholder="Type a secure message…" value={text}
            onChange={e => { const el = e.currentTarget; el.style.height = "auto"; el.style.height = Math.min(el.scrollHeight, 120) + "px"; setText(e.target.value); }}
            onKeyDown={e => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); submit(e); } if (e.key === "Escape" && !text) backToHub(); }}
            rows={1} disabled={connection?.state !== "established"} />
          {text.length > 64 * 1024 * 0.9 && <span className="msg-input-limit">{text.length}/{64 * 1024}</span>}
        </div>
        <div className="msg-input-timer">
          <select className="select--compact" value={timerSecs} onChange={e => setTimerSecs(parseInt(e.target.value, 10))}
            title="Self-destruct timer" aria-label="Self-destruct timer">
            <option value={0}>Off</option>
            <option value={5}>5s</option>
            <option value={30}>30s</option>
            <option value={60}>1m</option>
            <option value={300}>5m</option>
            <option value={3600}>1h</option>
            <option value={86400}>24h</option>
          </select>
        </div>
        <button type="submit" className="msg-send-btn" id="send-message-btn" disabled={!text.trim() || sending || connection?.state !== "established"}>
          {sending ? <span className="msg-send-spinner" /> : <SendIcon size={20} />}
        </button>
      </form>

      <div className="msg-footer">
        <span>End-to-end encrypted</span>
        <span>Ctrl+Enter to send · Esc to go back</span>
      </div>

      {/* Fingerprint Modal */}
      <Modal open={showFp} onClose={() => setShowFp(false)} title="Verify Peer Fingerprint"
        footer={!connection?.peer_verified ? <Button onClick={async () => { await handleVerify(); setShowFp(false); addToast("Peer verified", "success"); }}>Confirm Match & Verify</Button> : undefined}>
        <p className="fp-description">Compare fingerprints via a secure out-of-band channel.</p>
        <div className="fp-display">
          <div className="fp-side">
            <div className="fp-side__title">You (Local)</div>
            <div className="fp-grid">{identity?.fingerprint.split(":").map((g, i) => <span key={i} className="fp-grid__item">{g}</span>)}</div>
          </div>
          <div className="fp-compare">{connection?.peer_verified ? "Matched" : "Not yet verified"}</div>
          <div className="fp-side">
            <div className="fp-side__title">Peer</div>
            <div className="fp-grid">{connection?.peer_fingerprint?.split(":").map((g, i) => <span key={i} className="fp-grid__item">{g}</span>)}</div>
          </div>
        </div>
        {connection?.peer_verified && <p className="fp-success">Peer verified</p>}
      </Modal>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}

function formatMsg(content: string): React.ReactNode {
  const parts = content.split(/(`[^`]+`)/g);
  if (parts.length === 1) return content;
  return parts.map((p, i) => p.startsWith("`") && p.endsWith("`")
    ? <code key={i} className="msg-code-inline">{p.slice(1, -1)}</code>
    : p);
}

function SelfDestructTimer({ expiresAt }: { expiresAt: number }) {
  const [remaining, setRemaining] = useState(Math.max(0, expiresAt - Math.floor(Date.now() / 1000)));

  useEffect(() => {
    const timer = setInterval(() => {
      const r = Math.max(0, expiresAt - Math.floor(Date.now() / 1000));
      setRemaining(r);
      if (r <= 0) clearInterval(timer);
    }, 1000);
    return () => clearInterval(timer);
  }, [expiresAt]);

  if (remaining <= 0) return null;

  const mins = Math.floor(remaining / 60);
  const secs = remaining % 60;
  return (
    <span className="msg-timer" title={`Self-destructs in ${mins}m ${secs}s`}>
      🔥 {mins}:{secs.toString().padStart(2, "0")}
    </span>
  );
}

/** Simple markdown renderer: bold, italic, inline code, links */
function renderMarkdown(content: string): React.ReactNode {
  // Inline code first (so markdown inside backticks isn't parsed)
  const parts = content.split(/(`[^`]+`)/g);
  return parts.map((p, i) => {
    if (p.startsWith("`") && p.endsWith("`")) {
      return <code key={i} className="msg-code-inline">{p.slice(1, -1)}</code>;
    }
    // Bold **text** or __text__
    let rendered: React.ReactNode = p;
    const boldParts = p.split(/(\*\*[^*]+\*\*|__[^_]+__)/g);
    if (boldParts.length > 1) {
      rendered = boldParts.map((bp, j) => {
        if ((bp.startsWith("**") && bp.endsWith("**")) || (bp.startsWith("__") && bp.endsWith("__"))) {
          return <strong key={j}>{bp.slice(2, -2)}</strong>;
        }
        // Italic *text* or _text_
        const italicParts = bp.split(/(\*[^*]+\*|_[^_]+_)/g);
        if (italicParts.length > 1) {
          return italicParts.map((ip, k) => {
            if ((ip.startsWith("*") && ip.endsWith("*")) || (ip.startsWith("_") && ip.endsWith("_"))) {
              return <em key={k}>{ip.slice(1, -1)}</em>;
            }
            // Link detection (simple URL pattern)
            return renderLinks(ip, `${j}-${k}`);
          });
        }
        return renderLinks(bp, `${j}`);
      });
    } else {
      rendered = renderLinks(p, `${i}`);
    }
    return <span key={i}>{rendered}</span>;
  });
}

/** Detect URLs and render as clickable links */
function renderLinks(text: string, key: string): React.ReactNode {
  const urlRegex = /(https?:\/\/[^\s<]+)/g;
  const parts = text.split(urlRegex);
  if (parts.length === 1) return text;
  return parts.map((part, i) => {
    if (urlRegex.test(part)) {
      return <a key={`${key}-${i}`} href={part} target="_blank" rel="noopener noreferrer" className="msg-link">{part}</a>;
    }
    return part;
  });
}

function groupByDate(msgs: ChatMessage[]): Record<string, ChatMessage[]> {
  const g: Record<string, ChatMessage[]> = {};
  for (const m of msgs) {
    const d = new Date(m.timestamp * 1000), t = new Date(), y = new Date(t); y.setDate(y.getDate() - 1);
    const l = d.toDateString() === t.toDateString() ? "Today" : d.toDateString() === y.toDateString() ? "Yesterday" : d.toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" });
    if (!g[l]) g[l] = []; g[l].push(m);
  }
  return g;
}
