import { useState, useEffect, useRef } from "react";
import { ToastContainer } from "../components/ui";
import { useApp } from "../context/AppContext";
import { useChat } from "../context/ChatContext";
import type { ChatMessage } from "../types";

export default function ChatView() {
  const { toasts, removeToast, setView } = useApp();
  const {
    connection, messages, activeConversationId, typingPeers, conversations,
    handleSendMessage, handleDisconnect, handleMarkConversationRead,
    fileRequests, handleAcceptFileTransfer, handleRejectFileTransfer,
    handleSendFile, handleSendReaction, handleRemoveReaction,
    transfers,
  } = useChat();
  
  const [text, setText] = useState("");
  const [scrolledUp, setScrolledUp] = useState(false);
  const [sending, setSending] = useState(false);
  
  const msgRef = useRef<HTMLDivElement>(null);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => { 
    if (!scrolledUp && endRef.current) endRef.current.scrollIntoView({ behavior: "smooth" }); 
  }, [messages, scrolledUp]);

  useEffect(() => {
    const hasUnread = messages.some((m) => m.direction === "received" && m.read_at === null);
    if (hasUnread && activeConversationId) {
      const timer = setTimeout(() => handleMarkConversationRead(), 1000);
      return () => clearTimeout(timer);
    }
  }, [messages, activeConversationId, handleMarkConversationRead]);

  const submit = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!text.trim() || sending) return;
    setSending(true);
    try {
      await handleSendMessage(text.trim());
      setText("");
      if (endRef.current) endRef.current.scrollIntoView({ behavior: "smooth" });
    } catch {} finally { setSending(false); }
  };

  const grouped = groupByDate(messages);

  const currentConversation = conversations.find(c => c.peer_key_hex === activeConversationId || c.id === activeConversationId);
  const displayName = currentConversation?.display_name || currentConversation?.peer_display_name || connection?.peer_key_hex?.substring(0, 12) || "Secure Session";
  const firstLetter = displayName.charAt(0).toUpperCase();

  return (
    <main className="premium-glass-card w-full h-full flex flex-col relative z-10">
      {/* HEADER (64px) */}
      <header className="h-[64px] min-h-[64px] flex justify-between items-center px-xl border-b border-border-subtle bg-surface/80 backdrop-blur-3xl shrink-0">
        <div className="flex items-center gap-md">
          <button onClick={() => setView("hub")} className="text-on-surface-variant hover:text-primary transition-colors flex items-center p-xs hover:bg-bg-hover rounded-lg active:scale-95 transition-transform" title="Go back">
            <span className="material-symbols-outlined text-[22px]">arrow_back</span>
          </button>
          
          <div className="h-6 w-[1px] bg-border-subtle"></div>
          
          <div className="w-9 h-9 rounded-full bg-gradient-to-br from-primary to-[#a855f7] flex items-center justify-center font-bold text-white text-sm select-none">
            {firstLetter}
          </div>
          
          <div className="flex flex-col gap-0">
            <span className="font-bold text-on-surface text-body-lg leading-none">{displayName}</span>
            <div className="flex items-center gap-xs mt-1">
              <span className={`w-1.5 h-1.5 rounded-full ${connection?.state === "established" ? "bg-tertiary pulse-green" : "bg-warning"}`}></span>
              <span className="font-mono-label text-[10px] text-text-muted uppercase tracking-wider">
                {connection?.state === "established" ? "Online · E2EE" : connection?.state || "Connecting..."}
              </span>
            </div>
          </div>
        </div>
        
        <div className="flex items-center gap-md">
          {connection?.peer_verified && (
            <div className="flex items-center gap-xs px-sm py-1 bg-success-bg border border-success-glow text-tertiary rounded-lg select-none">
              <span className="material-symbols-outlined text-[14px]">verified_user</span>
              <span className="font-mono-label text-[9px] uppercase tracking-wider font-bold">Verified</span>
            </div>
          )}
          <button onClick={handleDisconnect} className="px-md py-1.5 border border-danger/25 text-danger hover:bg-danger/10 hover:border-danger/40 rounded-xl font-label-sm font-semibold active:scale-[0.96] transition-all">
            Disconnect
          </button>
        </div>
      </header>

      {/* FILE REQUEST BANNERS */}
      {fileRequests.length > 0 && fileRequests.map((req) => (
        <div key={req.transfer_id} className="px-xl py-lg border-b border-border-subtle bg-primary/5 backdrop-blur-xl shrink-0">
          <div className="flex items-center justify-between gap-md">
            <div className="flex items-center gap-md min-w-0">
              <div className="w-10 h-10 rounded-xl bg-primary/15 flex items-center justify-center shrink-0">
                <span className="material-symbols-outlined text-primary text-[20px]">description</span>
              </div>
              <div className="min-w-0">
                <p className="font-semibold text-on-surface text-body-md truncate">{req.filename}</p>
                <p className="font-label-xs text-[11px] text-text-muted">
                  {req.total_size > 1048576
                    ? `${(req.total_size / 1048576).toFixed(1)} MB`
                    : req.total_size > 1024
                    ? `${(req.total_size / 1024).toFixed(1)} KB`
                    : `${req.total_size} B`}
                  {" "}· From {connection?.peer_key_hex?.substring(0, 8) || "peer"}
                </p>
              </div>
            </div>
            <div className="flex items-center gap-sm shrink-0">
              <button
                onClick={() => handleRejectFileTransfer(req.transfer_id)}
                className="px-lg py-sm border border-outline-variant text-on-surface-variant hover:text-danger hover:border-danger/40 rounded-xl font-label-sm active:scale-95 transition-all"
              >
                Decline
              </button>
              <button
                onClick={() => handleAcceptFileTransfer(req.transfer_id)}
                className="px-lg py-sm bg-gradient-to-r from-primary to-inverse-primary text-white rounded-xl font-label-sm font-bold active:scale-95 transition-all shadow-[0_0_12px_rgba(99,102,241,0.3)]"
              >
                Accept
              </button>
            </div>
          </div>
        </div>
      ))}

      {/* TRANSFER PROGRESS BARS */}
      {transfers.map((t) => {
        const pct = t.total_size > 0 ? Math.min(100, Math.round((t.bytes_transferred / t.total_size) * 100)) : 0;
        const speed = t.speed_bytes_per_sec > 1048576
          ? `${(t.speed_bytes_per_sec / 1048576).toFixed(1)} MB/s`
          : t.speed_bytes_per_sec > 1024
          ? `${(t.speed_bytes_per_sec / 1024).toFixed(1)} KB/s`
          : `${t.speed_bytes_per_sec} B/s`;
        const remaining = t.estimated_remaining_secs > 3600
          ? `~${Math.round(t.estimated_remaining_secs / 3600)}h`
          : t.estimated_remaining_secs > 60
          ? `~${Math.round(t.estimated_remaining_secs / 60)}m`
          : t.estimated_remaining_secs > 0
          ? `~${t.estimated_remaining_secs}s`
          : "almost done";
        return (
          <div key={t.transfer_id} className="px-xl py-lg border-b border-border-subtle bg-primary/5 shrink-0">
            <div className="flex items-center justify-between gap-md mb-sm">
              <div className="flex items-center gap-md min-w-0">
                <div className="w-8 h-8 rounded-lg bg-primary/15 flex items-center justify-center shrink-0">
                  <span className="material-symbols-outlined text-primary text-[18px]">description</span>
                </div>
                <div className="min-w-0">
                  <p className="font-semibold text-on-surface text-body-sm truncate">{t.filename}</p>
                  <p className="font-label-xs text-[10px] text-text-muted">
                    {pct}% · {speed} · {remaining}
                  </p>
                </div>
              </div>
              <span className="font-bold text-primary text-body-sm shrink-0">{pct}%</span>
            </div>
            <div className="w-full h-1.5 rounded-full bg-white/5 overflow-hidden">
              <div
                className="h-full rounded-full bg-gradient-to-r from-primary to-inverse-primary transition-all duration-500 ease-out"
                style={{ width: `${pct}%` }}
              />
            </div>
          </div>
        );
      })}

      {/* MESSAGE AREA (scrollable) */}
      <section
        className="flex-1 overflow-y-auto custom-scrollbar p-xl flex flex-col gap-xl"
        ref={msgRef}
        onScroll={(e) => {
            const el = e.currentTarget;
            setScrolledUp(el.scrollHeight - el.scrollTop - el.clientHeight > 100);
        }}
      >
        {Object.entries(grouped).map(([label, msgs]) => (
          <div key={label} className="flex flex-col gap-xl">
            {/* Date Separator */}
            <div className="flex items-center gap-lg py-md opacity-40">
              <div className="h-[1px] flex-1 bg-gradient-to-r from-transparent to-outline"></div>
              <span className="font-label-sm text-label-sm text-on-surface-variant uppercase tracking-widest whitespace-nowrap">{label}</span>
              <div className="h-[1px] flex-1 bg-gradient-to-l from-transparent to-outline"></div>
            </div>
            
            {msgs.map((m: ChatMessage, idx: number) => {
              const isPrevSame = idx > 0 && msgs[idx - 1].direction === m.direction && (m.timestamp - msgs[idx - 1].timestamp) < 120;
              const isNextSame = idx < msgs.length - 1 && msgs[idx + 1].direction === m.direction && (msgs[idx + 1].timestamp - m.timestamp) < 120;

              const sentBubbleClass = !isPrevSame && !isNextSame
                ? "rounded-t-2xl rounded-bl-2xl rounded-br-sm"
                : !isPrevSame && isNextSame
                ? "rounded-t-2xl rounded-bl-2xl rounded-br-md"
                : isPrevSame && isNextSame
                ? "rounded-l-2xl rounded-r-sm"
                : "rounded-b-2xl rounded-l-2xl rounded-tr-sm";

              const receivedBubbleClass = !isPrevSame && !isNextSame
                ? "rounded-t-2xl rounded-br-2xl rounded-bl-sm"
                : !isPrevSame && isNextSame
                ? "rounded-t-2xl rounded-br-2xl rounded-bl-md"
                : isPrevSame && isNextSame
                ? "rounded-r-2xl rounded-l-sm"
                : "rounded-b-2xl rounded-r-2xl rounded-tl-sm";

              return m.direction === "sent" ? (
                <div key={m.id} className={`flex flex-col items-end gap-[2px] max-w-[75%] self-end animate-in slide-in-from-right-4 fade-in duration-300 ${isPrevSame ? "mt-0" : "mt-md"}`}>
                  <div className={`sent-bubble px-lg py-md bg-gradient-to-br from-primary to-inverse-primary ${sentBubbleClass} shadow-[0_4px_16px_rgba(99,102,241,0.2)] border border-outline-variant relative overflow-hidden group`}>
                    <div className="absolute inset-0 bg-white opacity-0 group-hover:opacity-10 transition-opacity duration-300 pointer-events-none"></div>
                    <p className="font-body-md text-white whitespace-pre-wrap break-words">{m.deleted ? <em className="opacity-50">Deleted</em> : m.content}</p>
                  {m.expires_at && !m.deleted && (
                    <div className="flex items-center gap-1 mt-xs opacity-60">
                      <span className="material-symbols-outlined text-[12px]">timer</span>
                      <span className="font-label-xs text-[10px]">{Math.max(0, Math.floor((m.expires_at - Math.floor(Date.now() / 1000)) / 60))}m</span>
                    </div>
                  )}
                  </div>
                  {!isNextSame && (
                    <span className="font-label-xs text-[10px] text-text-muted/70 px-xs tracking-wider mt-xs">
                      {new Date(m.timestamp * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                      {m.read_at && " ✓✓"}
                    </span>
                  )}
                  {/* Reaction buttons (show on hover) */}
                  <div className="flex items-center gap-1 mt-1 px-xs opacity-0 group-hover:opacity-100 transition-opacity duration-200">
                    {["👍", "❤️", "😂", "😮", "😢", "🙏"].map((emoji) => {
                      const hasReacted = m.reactions?.[emoji]?.includes("self") ?? false;
                      return (
                        <button
                          key={emoji}
                          onClick={(e) => { e.stopPropagation(); hasReacted ? handleRemoveReaction(m.id, emoji) : handleSendReaction(m.id, emoji); }}
                          className={`text-[14px] p-0.5 rounded-full transition-all hover:scale-125 ${hasReacted ? "bg-primary/20 scale-110" : "hover:bg-white/10"}`}
                          title={emoji}
                        >
                          {emoji}
                        </button>
                      );
                    })}
                  </div>
                  {/* Existing reactions display */}
                  {m.reactions && Object.keys(m.reactions).length > 0 && (
                    <div className="flex flex-wrap items-center gap-1 mt-1 px-xs">
                      {Object.entries(m.reactions).map(([emoji, reactors]) => (
                        <span key={emoji} className="flex items-center gap-0.5 text-[12px] bg-white/5 rounded-full px-1.5 py-0.5">
                          {emoji}
                          <span className="text-[10px] text-text-muted">{reactors.length}</span>
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              ) : (
                <div key={m.id} className={`flex flex-col items-start gap-[2px] max-w-[75%] self-start animate-in slide-in-from-left-4 fade-in duration-300 ${isPrevSame ? "mt-0" : "mt-md"}`}>
                  <div className={`received-bubble px-lg py-md bg-input-bg backdrop-blur-xl border border-outline-variant ${receivedBubbleClass} shadow-md hover:bg-bg-hover transition-colors duration-300`}>
                    <p className="font-body-md text-text-primary whitespace-pre-wrap break-words">{m.deleted ? <em className="opacity-50">Deleted</em> : m.content}</p>
                  {m.expires_at && !m.deleted && (
                    <div className="flex items-center gap-1 mt-xs opacity-60">
                      <span className="material-symbols-outlined text-[12px]">timer</span>
                      <span className="font-label-xs text-[10px]">{Math.max(0, Math.floor((m.expires_at - Math.floor(Date.now() / 1000)) / 60))}m</span>
                    </div>
                  )}
                  </div>
                  {!isNextSame && (
                    <span className="font-label-xs text-[10px] text-text-muted px-xs mt-xs">
                      {new Date(m.timestamp * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                    </span>
                  )}
                  {/* Reaction buttons (show on hover) */}
                  <div className="flex items-center gap-1 mt-1 px-xs opacity-0 group-hover:opacity-100 transition-opacity duration-200">
                    {["👍", "❤️", "😂", "😮", "😢", "🙏"].map((emoji) => {
                      const hasReacted = m.reactions?.[emoji]?.includes("self");
                      return (
                        <button
                          key={emoji}
                          onClick={(e) => { e.stopPropagation(); hasReacted ? handleRemoveReaction(m.id, emoji) : handleSendReaction(m.id, emoji); }}
                          className={`text-[14px] p-0.5 rounded-full transition-all hover:scale-125 ${hasReacted ? "bg-primary/20 scale-110" : "hover:bg-white/10"}`}
                          title={emoji}
                        >
                          {emoji}
                        </button>
                      );
                    })}
                  </div>
                  {/* Existing reactions display */}
                  {m.reactions && Object.keys(m.reactions).length > 0 && (
                    <div className="flex flex-wrap items-center gap-1 mt-1 px-xs">
                      {Object.entries(m.reactions).map(([emoji, reactors]) => (
                        <span key={emoji} className="flex items-center gap-0.5 text-[12px] bg-white/5 rounded-full px-1.5 py-0.5">
                          {emoji}
                          <span className="text-[10px] text-text-muted">{reactors.length}</span>
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        ))}
        {typingPeers.length > 0 && (
          <div className="flex flex-col items-start gap-xs max-w-[75%] self-start">
             <div className="received-bubble px-lg py-md shadow-lg flex gap-1">
               <span className="w-1.5 h-1.5 rounded-full bg-text-muted animate-pulse"></span>
               <span className="w-1.5 h-1.5 rounded-full bg-text-muted animate-pulse" style={{ animationDelay: '200ms' }}></span>
               <span className="w-1.5 h-1.5 rounded-full bg-text-muted animate-pulse" style={{ animationDelay: '400ms' }}></span>
             </div>
          </div>
        )}
        <div ref={endRef} />
      </section>

      {/* INPUT AREA */}
      <div className="p-xl border-t border-border-subtle input-blur shrink-0">
        <form onSubmit={submit} className="flex items-center gap-md bg-input-bg rounded-2xl p-sm border border-outline-variant">
          <div className="flex items-center gap-xs px-xs">
            <button type="button" onClick={handleSendFile} className="p-sm text-on-surface-variant hover:text-primary transition-colors">
              <span className="material-symbols-outlined text-[20px]">attach_file</span>
            </button>
            <button type="button" className="p-sm text-on-surface-variant hover:text-primary transition-colors">
              <span className="material-symbols-outlined text-[20px]">sentiment_satisfied</span>
            </button>
          </div>
          <textarea 
            value={text}
            onChange={(e) => {
                setText(e.target.value);
                e.target.style.height = '48px';
                e.target.style.height = `${e.target.scrollHeight}px`;
                e.target.style.overflowY = e.target.scrollHeight > 150 ? 'auto' : 'hidden';
            }}
            onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    submit();
                }
            }}
            disabled={connection?.state !== "established"}
            className="flex-1 bg-transparent border-none focus:ring-0 text-text-primary font-body-md py-md custom-scrollbar resize-none h-[48px]" 
            placeholder="Type a secure message..."
          ></textarea>
          <div className="flex items-center gap-md pr-sm">
            <button type="button" className="flex items-center gap-xs px-md py-1.5 bg-input-bg hover:bg-input-bg/80 rounded-full border border-border-subtle transition-all group">
              <span className="material-symbols-outlined text-[16px] text-warning">timer</span>
              <span className="font-label-sm text-label-sm text-on-surface-variant group-hover:text-text-primary transition-colors">Off</span>
            </button>
            <button type="submit" disabled={!text.trim() || sending} className="w-10 h-10 rounded-xl bg-gradient-to-tr from-primary to-inverse-primary text-white flex items-center justify-center disabled:opacity-50 transition-all duration-300 shadow-[0_0_15px_rgba(99,102,241,0.4)] hover:shadow-[0_0_25px_rgba(99,102,241,0.7)] active:scale-90 group shrink-0">
              <span className={`material-symbols-outlined text-[20px] ${sending ? 'animate-spin' : 'group-hover:-translate-y-0.5 group-hover:translate-x-0.5 transition-transform duration-300'}`}>{sending ? "sync" : "send"}</span>
            </button>
          </div>
        </form>
      </div>

      {/* FOOTER */}
      <footer className="px-xl py-lg border-t border-border-subtle bg-surface-container-lowest/50 flex justify-between items-center shrink-0">
        <div className="flex items-center gap-sm">
          <span className="material-symbols-outlined text-sm text-tertiary">lock</span>
          <span className="font-mono-label text-[10px] text-text-muted uppercase tracking-widest">End-to-end encrypted</span>
        </div>
        <span className="font-mono-label text-[10px] text-text-muted">Enter to send</span>
      </footer>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </main>
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
