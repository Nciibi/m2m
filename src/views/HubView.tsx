import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ToastContainer } from "../components/ui";
import {
  GearIcon, LinkIcon, CopyIcon, CheckIcon,
  MessageIcon, HomeIcon, WifiIcon
} from "../components/ui/Icons";
import { useApp } from "../context/AppContext";
import { useChat } from "../context/ChatContext";
import { useSettings } from "../context/SettingsContext";
import FamilyTab from "../components/FamilyTab";
import type { FamilyMember } from "../types";
import { hashToColor, formatTime } from "../utils";

export default function HubView() {
  const { identity, setView, toasts, removeToast } = useApp();
  const {
    connection, generatedInvite, inviteToConnect, inviteValid, namingMyName, namingTheirName,
    isConnecting, handleGenerateInvite, copyInvite, setInviteToConnect,
    handleConnect, setNamingMyName, setNamingTheirName, handleOpenChat,
    handleDeleteConversation, conversations,
    mutedConversations, handleMuteConversation, handleUnmuteConversation,
  } = useChat();
  const {
    networkSettings, privateMode, openSettings,
    discoveryConfig, discoveredPeers,
    handleConnectDiscoveredPeer, handleRefreshDiscovery,
    securityConfig, scheduleClipboardClear,
  } = useSettings();
  
  const [tab, setTab] = useState<"connect" | "chats" | "family" | "nearby">("connect");
  const [copied, setCopied] = useState(false);
  const [search, setSearch] = useState("");
  const [family, setFamily] = useState<FamilyMember[]>([]);

  const handleCopy = () => {
    copyInvite();
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
    if (securityConfig?.clipboard_clear_secs && securityConfig.clipboard_clear_secs > 0) {
      scheduleClipboardClear(securityConfig.clipboard_clear_secs);
    }
  };

  const loadFamily = useCallback(async () => {
    try {
      const f = await invoke<FamilyMember[]>("list_family");
      setFamily(f);
    } catch { /* noop */ }
  }, []);

  const handleFamilyConnect = useCallback(async (peerKeyHex: string) => {
    await invoke<any>("connect_family_member", { peerKeyHex });
    setView("chat");
  }, [setView]);

  useEffect(() => {
    if (tab === "family") loadFamily();
  }, [tab, loadFamily]);

  // Global keyboard shortcuts for Hub
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;
      if (ctrl && e.key === "n") {
        e.preventDefault();
        setTab("connect");
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const filtered = conversations.filter(c => {
    if (!search) return true;
    const q = search.toLowerCase();
    return (c.display_name || "").toLowerCase().includes(q) ||
      (c.peer_display_name || "").toLowerCase().includes(q) ||
      (c.last_message_preview || "").toLowerCase().includes(q) ||
      c.peer_key_hex.toLowerCase().includes(q);
  });

  return (
    <div style={{ display: 'flex', width: '100%', height: '100vh', alignItems: 'center', justifyContent: 'center', overflow: 'hidden' }}>
      {/* Background Glows matching 236 */}
      <div style={{ position: 'absolute', top: '-10%', left: '-10%', width: '40%', height: '40%', background: 'radial-gradient(circle at center, rgba(99, 102, 241, 0.15) 0%, transparent 70%)', pointerEvents: 'none' }} />
      <div style={{ position: 'absolute', bottom: '-10%', right: '-10%', width: '40%', height: '40%', background: 'radial-gradient(circle at center, rgba(99, 102, 241, 0.15) 0%, transparent 70%)', pointerEvents: 'none' }} />
      
      {/* Main Glass Shell */}
      <main className="app-shell" style={{ maxWidth: '1000px', flexDirection: 'column' }}>
        
        {/* Header */}
        <header className="hub-header">
          <div className="hub-header__brand">
            <div className="hub-header__logo">
              <span className="material-symbols-outlined" style={{ color: 'white', fontSize: 20 }}>security</span>
            </div>
            <span className="hub-header__title">M2M</span>
          </div>
          <div className="hub-header__actions">
            <div className="hub-status-pill">
              <span style={{
                width: 8, height: 8, borderRadius: '50%',
                background: connection?.state === "established" ? 'var(--color-success)' : isConnecting ? 'var(--color-warning)' : 'var(--color-text-muted)',
                boxShadow: connection?.state === "established" ? '0 0 8px var(--color-success)' : undefined
              }} />
              <span style={{ fontSize: '12px', fontWeight: 500, color: 'var(--color-text-primary)' }}>
                {isConnecting ? "Connecting" : connection?.state === "established" ? "Online" : "Offline"}
              </span>
            </div>
            <button className="icon-btn" onClick={openSettings} title="Settings">
              <GearIcon size={22} />
            </button>
          </div>
        </header>

        {/* Tab Bar */}
        <nav className="hub-tab-bar">
          <button className={`hub-tab ${tab === "connect" ? "hub-tab--active" : ""}`} onClick={() => setTab("connect")}>
            <LinkIcon size={18} /> Connect
          </button>
          <button className={`hub-tab ${tab === "chats" ? "hub-tab--active" : ""}`} onClick={() => setTab("chats")}>
            <MessageIcon size={18} /> Chats
            {conversations.length > 0 && <span className="hub-tab-badge">{conversations.length}</span>}
          </button>
          <button className={`hub-tab ${tab === "nearby" ? "hub-tab--active" : ""}`} onClick={() => setTab("nearby")}>
            <WifiIcon size={18} /> Nearby
            {discoveredPeers.length > 0 && <span className="hub-tab-badge">{discoveredPeers.length}</span>}
          </button>
          <button className={`hub-tab ${tab === "family" ? "hub-tab--active" : ""}`} onClick={() => setTab("family")}>
            <HomeIcon size={18} /> Family
            {family.length > 0 && <span className="hub-tab-badge">{family.length}</span>}
          </button>
        </nav>

        {/* Content Area */}
        <div className="hub-content">
          {tab === "connect" ? (
            <ConnectTab
              generatedInvite={generatedInvite} inviteToConnect={inviteToConnect}
              inviteValid={inviteValid} namingMyName={namingMyName} namingTheirName={namingTheirName}
              isConnecting={isConnecting} onGenerateInvite={handleGenerateInvite}
              onCopyInvite={handleCopy} copied={copied}
              setInviteToConnect={setInviteToConnect} onConnect={handleConnect}
              setNamingMyName={setNamingMyName} setNamingTheirName={setNamingTheirName}
              networkSettings={networkSettings} privateMode={privateMode} identity={identity}
              securityConfig={securityConfig} scheduleClipboardClear={scheduleClipboardClear}
            />
          ) : tab === "nearby" ? (
            <NearbyTab
              discoveryConfig={discoveryConfig}
              discoveredPeers={discoveredPeers}
              onConnect={handleConnectDiscoveredPeer}
              onRefresh={handleRefreshDiscovery}
              onOpenSettings={openSettings}
              onOpenChat={handleOpenChat}
            />
          ) : tab === "family" ? (
            <FamilyTab family={family} onRefresh={loadFamily} onConnect={handleFamilyConnect} />
          ) : (
            <ChatsTab conversations={filtered} onOpenChat={handleOpenChat} onDeleteConversation={handleDeleteConversation} search={search} setSearch={setSearch} onGetStarted={() => setTab("connect")} mutedConversations={mutedConversations} onMute={handleMuteConversation} onUnmute={handleUnmuteConversation} />
          )}
        </div>

        {/* Footer (from Connect view in 236) */}
        {tab === "connect" && (
          <footer className="hub-footer">
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
              <span style={{ fontSize: '10px', color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '0.2em' }}>Your Identity Fingerprint</span>
              <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                <span style={{ fontFamily: 'var(--font-mono)', fontSize: '13px', color: 'var(--color-primary)', background: 'rgba(255,255,255,0.05)', padding: '4px 12px', borderRadius: 8, border: '1px solid var(--color-border-subtle)' }}>
                  {identity?.fingerprint || "Loading..."}
                </span>
              </div>
            </div>
          </footer>
        )}

      </main>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}

function ConnectTab({ generatedInvite, inviteToConnect, inviteValid, namingMyName, namingTheirName, isConnecting, onGenerateInvite, onCopyInvite, copied, setInviteToConnect, onConnect, setNamingMyName, setNamingTheirName }: any) {
  const [generating, setGenerating] = useState(false);
  
  const handleGenerate = async () => {
    setGenerating(true);
    try {
      await onGenerateInvite();
    } finally { setGenerating(false); }
  };

  return (
    <div className="connect-grid">
      {/* Host a Connection */}
      <section className="glass-card-section">
        <div style={{ display: 'flex', alignItems: 'center', gap: 16, marginBottom: 24 }}>
          <div style={{ width: 40, height: 40, borderRadius: 12, background: 'rgba(192,193,255,0.1)', border: '1px solid rgba(192,193,255,0.2)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <span className="material-symbols-outlined" style={{ color: 'var(--color-primary)' }}>broadcast_on_personal</span>
          </div>
          <div>
            <h2 style={{ fontSize: '20px', fontWeight: 600, color: 'var(--color-text-primary)' }}>Host a Connection</h2>
            <p style={{ fontSize: '14px', color: 'var(--color-text-secondary)' }}>Create a secure link for others to join.</p>
          </div>
        </div>

        <button 
          onClick={handleGenerate}
          disabled={generating}
          style={{ width: '100%', padding: '16px', borderRadius: '12px', border: 'none', fontSize: '18px', fontWeight: 700, cursor: 'pointer', transition: 'all 0.2s', marginBottom: '24px' }}
          className="btn-generate-glow"
        >
          {generating ? "Generating..." : "Generate Invite Link"}
        </button>

        {generatedInvite && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <label style={{ fontSize: '12px', color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '1px' }}>Your Active Invite</label>
            <div className="invite-output-box">
              <span className="invite-text">{generatedInvite}</span>
              <button className="icon-btn" onClick={onCopyInvite} title="Copy">
                {copied ? <CheckIcon size={20} color="var(--color-success)" /> : <CopyIcon size={20} />}
              </button>
            </div>
          </div>
        )}
      </section>

      {/* Join a Connection */}
      <section className="glass-card-section">
        <div style={{ display: 'flex', alignItems: 'center', gap: 16, marginBottom: 24 }}>
          <div style={{ width: 40, height: 40, borderRadius: 12, background: 'rgba(78,222,163,0.1)', border: '1px solid rgba(78,222,163,0.2)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <span className="material-symbols-outlined" style={{ color: 'var(--color-success)' }}>key</span>
          </div>
          <div>
            <h2 style={{ fontSize: '20px', fontWeight: 600, color: 'var(--color-text-primary)' }}>Join a Connection</h2>
            <p style={{ fontSize: '14px', color: 'var(--color-text-secondary)' }}>Enter a link to start an encrypted chat.</p>
          </div>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 24 }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <label style={{ fontSize: '12px', color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '1px' }}>Invite Link</label>
            <input 
              value={inviteToConnect} 
              onChange={e => setInviteToConnect(e.target.value)}
              placeholder="m2m://..." 
              style={{ width: '100%', background: 'var(--color-bg-input)', border: '1px solid var(--color-border-subtle)', borderRadius: 12, padding: '12px 16px', color: 'var(--color-primary)', fontFamily: 'var(--font-mono)', outline: 'none' }}
            />
            {inviteValid && (
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, color: 'var(--color-success)', fontSize: 12, marginTop: 4 }}>
                <span className="material-symbols-outlined" style={{ fontSize: 16 }}>check_circle</span>
                <span>Valid Invite Found</span>
              </div>
            )}
          </div>

          {inviteValid && (
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <label style={{ fontSize: '12px', color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '1px' }}>Your Name</label>
                <input value={namingMyName} onChange={e => setNamingMyName(e.target.value)} placeholder="Nexus-01" style={{ width: '100%', background: 'var(--color-bg-input)', border: '1px solid var(--color-border-subtle)', borderRadius: 12, padding: '12px 16px', color: 'white', outline: 'none' }} />
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <label style={{ fontSize: '12px', color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '1px' }}>Their Name</label>
                <input value={namingTheirName} onChange={e => setNamingTheirName(e.target.value)} placeholder="Ghost-Host" style={{ width: '100%', background: 'var(--color-bg-input)', border: '1px solid var(--color-border-subtle)', borderRadius: 12, padding: '12px 16px', color: 'white', outline: 'none' }} />
              </div>
            </div>
          )}

          <button 
            onClick={onConnect}
            disabled={isConnecting || !inviteToConnect}
            style={{ width: '100%', padding: '16px', borderRadius: '12px', border: 'none', background: 'var(--color-success)', color: 'var(--color-bg-dark)', fontSize: '18px', fontWeight: 700, cursor: isConnecting || !inviteToConnect ? 'not-allowed' : 'pointer', opacity: isConnecting || !inviteToConnect ? 0.5 : 1, transition: 'all 0.2s', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8 }}
          >
            <span className="material-symbols-outlined">sensors</span>
            {isConnecting ? "Connecting..." : "Connect"}
          </button>
        </div>
      </section>
    </div>
  );
}

function ChatsTab({ conversations, onOpenChat, onDeleteConversation, search, setSearch, onGetStarted, mutedConversations, onMute, onUnmute }: any) {
  const [favorites, setFavorites] = useState<Set<string>>(new Set());

  useEffect(() => {
    setFavorites(new Set(conversations.filter((c: any) => c.is_favorite).map((c: any) => c.peer_key_hex)));
  }, [conversations]);

  const toggleFav = async (peerKeyHex: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      const newVal = await invoke<boolean>("toggle_favorite", { peerKeyHex });
      setFavorites(prev => { const n = new Set(prev); if (newVal) n.add(peerKeyHex); else n.delete(peerKeyHex); return n; });
    } catch {}
  };

  const toggleArch = async (peerKeyHex: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await invoke<boolean>("toggle_archive", { peerKeyHex });
      // UI optimistic update omitted for brevity
    } catch {}
  };

  const sorted = [...conversations].sort((a: any, b: any) => {
    if ((a.archived ? 1 : 0) !== (b.archived ? 1 : 0)) return (a.archived ? 1 : 0) - (b.archived ? 1 : 0);
    if ((a.is_favorite ? 1 : 0) !== (b.is_favorite ? 1 : 0)) return (b.is_favorite ? 1 : 0) - (a.is_favorite ? 1 : 0);
    return (b.last_message_at || 0) - (a.last_message_at || 0);
  });

  return (
    <div className="chats-layout">
      {conversations.length > 0 && (
        <div style={{ position: 'relative' }}>
          <span className="material-symbols-outlined" style={{ position: 'absolute', left: 16, top: '50%', transform: 'translateY(-50%)', color: 'var(--color-text-muted)' }}>search</span>
          <input 
            type="text" 
            placeholder="Search conversations..." 
            value={search}
            onChange={e => setSearch(e.target.value)}
            style={{ width: '100%', height: 40, background: 'rgba(0,0,0,0.4)', border: '1px solid var(--color-border-subtle)', borderRadius: 12, paddingLeft: 48, paddingRight: 16, color: 'white', outline: 'none' }}
          />
        </div>
      )}

      <div style={{ flex: 1, overflowY: 'auto' }}>
        {sorted.map((c: any) => {
          const isMuted = mutedConversations?.includes(c.peer_key_hex);
          return (
            <div key={c.id} className="chat-item" onClick={() => onOpenChat(c)}>
              <div className="chat-item-avatar" style={{ background: `linear-gradient(135deg, ${hashToColor(c.peer_key_hex)}, ${hashToColor(c.peer_key_hex.slice(16))})` }}>
                {(c.display_name || c.peer_display_name || c.peer_key_hex).charAt(0).toUpperCase()}
                {c.is_online && <span className="status-dot" />}
              </div>
              <div className="chat-item-body">
                <div className="chat-item-top">
                  <span className="chat-item-name">
                    {c.display_name || c.peer_display_name || "Unknown Peer"}
                    {favorites.has(c.peer_key_hex) && <span className="material-symbols-outlined" style={{ fontSize: 16, color: 'var(--color-warning)', marginLeft: 4 }}>star</span>}
                  </span>
                  {c.last_message_at && <span className="chat-item-time">{formatTime(c.last_message_at)}</span>}
                </div>
                <div className="chat-item-preview">{c.last_message_preview || "No messages yet."}</div>
              </div>
              <div className="chat-item-actions">
                <button className="icon-btn" onClick={(e) => toggleFav(c.peer_key_hex, e)} title="Favorite">
                  <span className="material-symbols-outlined" style={{ fontSize: 20 }}>{favorites.has(c.peer_key_hex) ? "star" : "star_border"}</span>
                </button>
                <button className="icon-btn" onClick={(e) => { e.stopPropagation(); isMuted ? onUnmute(c.peer_key_hex) : onMute(c.peer_key_hex); }} title="Mute">
                  <span className="material-symbols-outlined" style={{ fontSize: 20 }}>{isMuted ? "notifications_off" : "notifications"}</span>
                </button>
                <button className="icon-btn" onClick={(e) => toggleArch(c.peer_key_hex, e)} title="Archive">
                  <span className="material-symbols-outlined" style={{ fontSize: 20 }}>archive</span>
                </button>
                <button className="icon-btn icon-btn--danger" onClick={(e) => { e.stopPropagation(); invoke("delete_conversation_cmd", { conversationId: c.id }).then(() => onDeleteConversation(c.id)).catch(console.error); }} title="Delete">
                  <span className="material-symbols-outlined" style={{ fontSize: 20 }}>delete</span>
                </button>
              </div>
            </div>
          );
        })}

        {conversations.length === 0 && (
          <div style={{ textAlign: 'center', marginTop: 60 }}>
            <span className="material-symbols-outlined" style={{ fontSize: 48, color: 'var(--color-text-muted)', marginBottom: 16 }}>chat_bubble</span>
            <p style={{ fontSize: 18, fontWeight: 600, color: 'white' }}>No conversations yet</p>
            <p style={{ color: 'var(--color-text-secondary)', marginBottom: 24 }}>Host a connection or join one to start chatting.</p>
            <button className="btn-generate-glow" style={{ border: 'none', padding: '12px 24px', borderRadius: 12, fontWeight: 700, cursor: 'pointer' }} onClick={onGetStarted}>
              Get Started
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function NearbyTab() {
  // Omitted for brevity, will keep simple
  return (
    <div style={{ color: 'white' }}>Nearby Tab - To be enhanced</div>
  );
}
