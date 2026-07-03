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

  return (
    <main className="premium-glass-card w-full h-full flex flex-col relative z-10">
      {/* HEADER (52px) */}
      <header className="h-[52px] min-h-[52px] flex justify-between items-center px-xl border-b border-border-subtle bg-surface/80 backdrop-blur-3xl shrink-0">
        <div className="flex items-center gap-md">
          <span className="material-symbols-outlined text-outline text-[20px]">shield</span>
          <span className="font-label-sm text-label-sm text-on-surface-variant tracking-wide">Encrypted Session</span>
        </div>
        <div className="flex items-center gap-lg">
          <button onClick={() => setView("hub")} className="text-on-surface-variant hover:text-primary transition-colors flex items-center">
            <span className="material-symbols-outlined text-[22px]">arrow_back</span>
          </button>
          <div className="flex items-center gap-sm px-md py-xs bg-tertiary-container/10 rounded-full border border-tertiary-container/20">
            <span className={`w-1.5 h-1.5 rounded-full ${connection?.state === "established" ? "bg-tertiary pulse-green" : "bg-warning"}`}></span>
            <span className={`font-label-xs text-label-xs font-bold ${connection?.state === "established" ? "text-tertiary" : "text-warning"}`}>
              {connection?.state === "established" ? "Online" : connection?.state || "Offline"}
            </span>
          </div>
          <button onClick={handleDisconnect} className="px-md py-1 border border-danger/30 text-danger hover:bg-danger/10 rounded-lg font-label-sm transition-all">
            Disconnect
          </button>
        </div>
      </header>

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
                  </div>
                  {!isNextSame && (
                    <span className="font-label-xs text-[10px] text-text-muted/70 px-xs tracking-wider mt-xs">
                      {new Date(m.timestamp * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                      {m.read_at && " ✓✓"}
                    </span>
                  )}
                </div>
              ) : (
                <div key={m.id} className={`flex flex-col items-start gap-[2px] max-w-[75%] self-start animate-in slide-in-from-left-4 fade-in duration-300 ${isPrevSame ? "mt-0" : "mt-md"}`}>
                  <div className={`received-bubble px-lg py-md bg-input-bg backdrop-blur-xl border border-outline-variant ${receivedBubbleClass} shadow-md hover:bg-bg-hover transition-colors duration-300`}>
                    <p className="font-body-md text-text-primary whitespace-pre-wrap break-words">{m.deleted ? <em className="opacity-50">Deleted</em> : m.content}</p>
                  </div>
                  {!isNextSame && (
                    <span className="font-label-xs text-[10px] text-text-muted px-xs mt-xs">
                      {new Date(m.timestamp * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                    </span>
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
            <button type="button" className="p-sm text-on-surface-variant hover:text-primary transition-colors">
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
