import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { Button, Badge, Modal, ToastContainer } from "../components/ui";
import {
  ArrowLeftIcon, ShieldIcon, VerifiedIcon, LockIcon,
  SendIcon, AttachIcon, FileIcon, ArrowDownIcon,
} from "../components/ui/Icons";
import { useApp } from "../context/AppContext";
import { useChat } from "../context/ChatContext";
import type { ChatMessage } from "../types";

export default function ChatView() {
  const { identity, toasts, removeToast, addToast, setView } = useApp();
  const {
    connection, messages, fileRequests, activeConversationId,
    handleSendMessage, handleSendFile, handleVerify, handleDisconnect,
    handleExportConversation, handleSetRetention,
    retentionPolicy, setRetentionPolicy, retentionDuration, setRetentionDuration,
    handleSendReaction, handleRemoveReaction, handleMarkConversationRead,
  } = useChat();
  const [text, setText] = useState("");
  const [showFp, setShowFp] = useState(false);
  const [scrolledUp, setScrolledUp] = useState(false);
  const [sending, setSending] = useState(false);
  const [pickerMsgId, setPickerMsgId] = useState<string | null>(null);
  const endRef = useRef<HTMLDivElement>(null);
  const msgRef = useRef<HTMLDivElement>(null);

  useEffect(() => { if (!scrolledUp && msgRef.current) msgRef.current.scrollTop = msgRef.current.scrollHeight; }, [messages, scrolledUp]);

  const onScroll = () => {
    const el = msgRef.current;
    if (!el) return;
    setScrolledUp(el.scrollHeight - el.scrollTop - el.clientHeight > 100);
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!text.trim() || sending) return;
    setSending(true);
    try { await handleSendMessage(text.trim().slice(0, 64 * 1024)); setText(""); } finally { setSending(false); }
  };

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
          <Badge variant={connection?.state === "established" ? "success" : "danger"} dot compact>{connection?.state || "unknown"}</Badge>
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
            {msgs.map((m: any, i: number) => (
              <div key={m.id} className={`msg-bubble msg-bubble--${m.direction}`} style={{ animationDelay: `${i * 0.05}s` }}>
                {formatMsg(m.content)}
                <span className="msg-time">{new Date(m.timestamp * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
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

function groupByDate(msgs: ChatMessage[]): Record<string, ChatMessage[]> {
  const g: Record<string, ChatMessage[]> = {};
  for (const m of msgs) {
    const d = new Date(m.timestamp * 1000), t = new Date(), y = new Date(t); y.setDate(y.getDate() - 1);
    const l = d.toDateString() === t.toDateString() ? "Today" : d.toDateString() === y.toDateString() ? "Yesterday" : d.toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" });
    if (!g[l]) g[l] = []; g[l].push(m);
  }
  return g;
}
