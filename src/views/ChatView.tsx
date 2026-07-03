import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { ToastContainer } from "../components/ui";
import { useApp } from "../context/AppContext";
import { useChat } from "../context/ChatContext";
import type { ChatMessage } from "../types";
import { hashToColor } from "../utils";

export default function ChatView() {
  const { identity, toasts, removeToast, addToast, setView } = useApp();
  const {
    connection, messages, setMessages, fileRequests, activeConversationId, typingPeers,
    reconnecting, reconnectAttempt,
    handleSendMessage, handleSendMessageWithTimer, handleSendFile, handleVerify, handleDisconnect,
    handleReconnect, handleExportConversation, handleSetRetention,
    retentionPolicy, setRetentionPolicy, retentionDuration, setRetentionDuration,
    handleSendReaction, handleRemoveReaction, handleMarkConversationRead,
    handleEditMessage, handleDeleteMessage,
  } = useChat();
  
  const [text, setText] = useState("");
  const [showFp, setShowFp] = useState(false);
  const [scrolledUp, setScrolledUp] = useState(false);
  const [sending, setSending] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [hasOlder, setHasOlder] = useState(true);
  
  const msgRef = useRef<HTMLDivElement>(null);
  const endRef = useRef<HTMLDivElement>(null);

  // Auto-scroll logic
  useEffect(() => { 
    if (!scrolledUp && endRef.current) endRef.current.scrollIntoView({ behavior: "smooth" }); 
  }, [messages, scrolledUp]);

  // Mark as read
  useEffect(() => {
    const hasUnread = messages.some((m) => m.direction === "received" && m.read_at === null);
    if (hasUnread && activeConversationId) {
      const timer = setTimeout(() => handleMarkConversationRead(), 1000);
      return () => clearTimeout(timer);
    }
  }, [messages, activeConversationId, handleMarkConversationRead]);

  // Send message
  const submit = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!text.trim() || sending) return;
    setSending(true);
    try {
      await handleSendMessage(text.trim());
      setText("");
    } catch {} finally { setSending(false); }
  };

  const backToHub = () => setView("hub");

  const grouped = groupByDate(messages);

  return (
    <div style={{ display: 'flex', width: '100%', minHeight: '100vh', alignItems: 'center', justifyContent: 'center', background: 'var(--color-bg-dark)', overflow: 'hidden', position: 'relative' }}>
      {/* Background Glows */}
      <div style={{ position: 'absolute', top: '10%', right: '-20%', width: '60%', height: '60%', background: 'radial-gradient(circle, rgba(99, 102, 241, 0.1) 0%, transparent 60%)', pointerEvents: 'none' }} />
      <div style={{ position: 'absolute', bottom: '-20%', left: '-10%', width: '50%', height: '50%', background: 'radial-gradient(circle, rgba(16, 185, 129, 0.05) 0%, transparent 60%)', pointerEvents: 'none' }} />

      <main style={{ 
        width: '100%', height: '100dvh', display: 'flex', flexDirection: 'column', position: 'relative', zIndex: 10,
        maxWidth: '1000px', margin: 'auto', background: 'rgba(12, 14, 24, 0.85)', backdropFilter: 'blur(32px)',
        border: '1px solid rgba(255, 255, 255, 0.05)', borderRadius: '24px', boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.5)', overflow: 'hidden'
      }}>
        
        {/* Header */}
        <header style={{ height: '64px', padding: '0 24px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid rgba(255, 255, 255, 0.05)', background: 'rgba(255, 255, 255, 0.02)', flexShrink: 0 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
            <button onClick={backToHub} style={{ background: 'transparent', border: 'none', color: 'var(--color-text-secondary)', cursor: 'pointer', display: 'flex', alignItems: 'center' }}>
              <span className="material-symbols-outlined">arrow_back</span>
            </button>
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <div style={{ width: '40px', height: '40px', borderRadius: '50%', background: `linear-gradient(135deg, ${hashToColor(activeConversationId || '')}, ${hashToColor((activeConversationId || '').slice(16))})`, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'white', fontWeight: 700 }}>
                {(activeConversationId || '?').charAt(0).toUpperCase()}
              </div>
              <div>
                <h2 style={{ fontSize: '16px', fontWeight: 600, color: 'var(--color-text-primary)', margin: 0, display: 'flex', alignItems: 'center', gap: '6px' }}>
                  Peer Contact
                  {connection?.peer_verified && <span className="material-symbols-outlined" style={{ fontSize: '14px', color: 'var(--color-success)' }}>verified</span>}
                </h2>
                <div style={{ fontSize: '12px', color: connection?.state === "established" ? 'var(--color-success)' : 'var(--color-warning)', display: 'flex', alignItems: 'center', gap: '4px' }}>
                  <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: 'currentColor' }} />
                  {connection?.state === "established" ? "Online & Encrypted" : connection?.state || "Offline"}
                </div>
              </div>
            </div>
          </div>
          <div style={{ display: 'flex', gap: '12px' }}>
            <button className="icon-btn" onClick={() => setShowFp(true)} title="Verify Fingerprint">
              <span className="material-symbols-outlined">security</span>
            </button>
            <button className="icon-btn" style={{ color: 'var(--color-danger)' }} onClick={handleDisconnect} title="Disconnect">
              <span className="material-symbols-outlined">power_settings_new</span>
            </button>
          </div>
        </header>

        {/* Messages Area */}
        <div 
          ref={msgRef} 
          style={{ flex: 1, overflowY: 'auto', padding: '24px', display: 'flex', flexDirection: 'column', gap: '16px' }}
          onScroll={(e) => {
            const el = e.currentTarget;
            setScrolledUp(el.scrollHeight - el.scrollTop - el.clientHeight > 100);
          }}
        >
          <div style={{ textAlign: 'center', marginBottom: '24px' }}>
            <span style={{ background: 'rgba(255,255,255,0.05)', padding: '6px 16px', borderRadius: '12px', fontSize: '12px', color: 'var(--color-text-muted)', display: 'inline-flex', alignItems: 'center', gap: '8px' }}>
              <span className="material-symbols-outlined" style={{ fontSize: '16px', color: 'var(--color-warning)' }}>lock</span>
              End-to-End Encrypted Session
            </span>
          </div>

          {Object.entries(grouped).map(([label, msgs]) => (
            <div key={label} style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div className="chat-date-separator">{label}</div>
              {msgs.map((m: ChatMessage) => (
                <div key={m.id} className={`chat-bubble chat-bubble-${m.direction}`}>
                  {m.deleted ? (
                    <em style={{ opacity: 0.5 }}>Message deleted</em>
                  ) : (
                    <div>{m.content}</div>
                  )}
                  <div className="chat-bubble-time">
                    {new Date(m.timestamp * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                    {m.direction === "sent" && m.read_at && <span style={{ marginLeft: '4px', color: '#4ade80' }}>✓✓</span>}
                  </div>
                </div>
              ))}
            </div>
          ))}

          {typingPeers.length > 0 && (
            <div style={{ display: 'flex', gap: '8px', padding: '12px', background: 'rgba(255,255,255,0.05)', borderRadius: '16px', alignSelf: 'flex-start', width: 'fit-content' }}>
              <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: 'var(--color-text-muted)', animation: 'dotBounce 1.4s ease-in-out infinite both' }} />
              <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: 'var(--color-text-muted)', animation: 'dotBounce 1.4s ease-in-out infinite both', animationDelay: '0.16s' }} />
              <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: 'var(--color-text-muted)', animation: 'dotBounce 1.4s ease-in-out infinite both', animationDelay: '0.32s' }} />
            </div>
          )}
          <div ref={endRef} />
        </div>

        {/* Input Area */}
        <footer style={{ padding: '24px', background: 'rgba(12, 14, 24, 0.9)', borderTop: '1px solid rgba(255, 255, 255, 0.05)' }}>
          <form onSubmit={submit} className="chat-input-wrapper">
            <button type="button" className="chat-action-btn chat-attach-btn" onClick={handleSendFile}>
              <span className="material-symbols-outlined">attach_file</span>
            </button>
            <textarea
              value={text}
              onChange={e => setText(e.target.value)}
              onKeyDown={e => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); submit(); } }}
              placeholder="Message..."
              className="chat-input"
              rows={1}
              disabled={connection?.state !== "established"}
            />
            <button type="submit" disabled={!text.trim() || sending || connection?.state !== "established"} className="chat-action-btn chat-send-btn">
              {sending ? <span className="material-symbols-outlined" style={{ animation: 'spin 1s linear infinite' }}>sync</span> : <span className="material-symbols-outlined">send</span>}
            </button>
          </form>
        </footer>
      </main>

      {/* Fingerprint Verification Modal */}
      {showFp && (
        <div style={{ position: 'fixed', inset: 0, zIndex: 9999, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(0,0,0,0.8)', backdropFilter: 'blur(8px)' }}>
          <div style={{ background: 'var(--color-bg-surface-variant)', border: '1px solid var(--color-border-subtle)', borderRadius: '24px', padding: '32px', maxWidth: '400px', width: '100%', boxShadow: '0 25px 50px -12px rgba(0,0,0,0.5)' }}>
            <h3 style={{ margin: '0 0 16px 0', fontSize: '20px', color: 'white' }}>Verify Identity</h3>
            <p style={{ color: 'var(--color-text-secondary)', fontSize: '14px', marginBottom: '24px' }}>Compare this fingerprint with your peer out-of-band.</p>
            <div style={{ background: 'rgba(0,0,0,0.3)', padding: '16px', borderRadius: '12px', fontFamily: 'var(--font-mono)', fontSize: '12px', color: 'var(--color-primary)', wordBreak: 'break-all', marginBottom: '24px' }}>
              {connection?.peer_fingerprint || "Unknown"}
            </div>
            <div style={{ display: 'flex', gap: '12px', justifyContent: 'flex-end' }}>
              <button onClick={() => setShowFp(false)} style={{ background: 'transparent', color: 'white', border: '1px solid rgba(255,255,255,0.1)', padding: '8px 16px', borderRadius: '8px', cursor: 'pointer' }}>Cancel</button>
              <button onClick={() => { handleVerify(); setShowFp(false); }} style={{ background: 'var(--color-success)', color: 'var(--color-bg-dark)', border: 'none', padding: '8px 16px', borderRadius: '8px', fontWeight: 600, cursor: 'pointer' }}>Verify Match</button>
            </div>
          </div>
        </div>
      )}

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
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
