import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, Input, Card, Badge, ToastContainer } from "../components/ui";
import {
  ShieldIcon, GearIcon, PlusIcon, LinkIcon, CopyIcon, CheckIcon,
  SearchIcon, MessageIcon, TrashIcon, OnlineDot, OfflineDot, HomeIcon, WifiIcon,
} from "../components/ui/Icons";
import { useApp } from "../context/AppContext";
import { useChat } from "../context/ChatContext";
import { useSettings } from "../context/SettingsContext";
import FamilyTab from "../components/FamilyTab";
import type { FamilyMember } from "../types";

export default function HubView() {
  const { identity, setView, toasts, removeToast } = useApp();
  const {
    connection, generatedInvite, inviteToConnect, inviteValid, namingMyName, namingTheirName,
    isConnecting, handleGenerateInvite, copyInvite, setInviteToConnect,
    handleConnect, setNamingMyName, setNamingTheirName, handleOpenChat,
    handleDeleteConversation, conversations,
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
  const [_familyLoading, setFamilyLoading] = useState(false);

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
      setFamilyLoading(true);
      const f = await invoke<FamilyMember[]>("list_family");
      setFamily(f);
    } catch { /* noop */ }
    finally { setFamilyLoading(false); }
  }, []);

  const handleFamilyConnect = useCallback(async (peerKeyHex: string) => {
    // connect emits m2m://connection event which ChatContext picks up
    await invoke<any>("connect_family_member", { peerKeyHex });
    setView("chat");
  }, [setView]);

  // Load family on mount and when switching to family tab
  useEffect(() => {
    if (tab === "family") loadFamily();
  }, [tab, loadFamily]);

  // Derive connection state for the status badge
  const connectionBadge = (() => {
    if (isConnecting) return { dot: null, label: "Connecting…", variant: "warning" as const };
    if (connection?.state === "established") return { dot: <OnlineDot />, label: "Connected", variant: "success" as const };
    return { dot: <OfflineDot />, label: "Offline", variant: "default" as const };
  })();

  const filtered = conversations.filter(c => {
    if (!search) return true;
    const q = search.toLowerCase();
    return (c.display_name || "").toLowerCase().includes(q) ||
      (c.peer_display_name || "").toLowerCase().includes(q) ||
      (c.last_message_preview || "").toLowerCase().includes(q) ||
      c.peer_key_hex.toLowerCase().includes(q);
  });

  return (
    <div className="app-shell">
      <div className="app-header">
        <h1 className="app-header__title">
          <span className="app-header__icon-bg app-header__icon-bg--accent">
            <img src="logo.png" alt="M2M" width="20" height="20" style={{ borderRadius: '4px' }} />
          </span>
          M2M
        </h1>
        </h1>
        <div className="app-header__actions">
          <Badge variant={connectionBadge.variant} compact>
            {connectionBadge.dot} {connectionBadge.label}
          </Badge>
          <button className="btn btn--icon" onClick={openSettings} id="settings-btn" aria-label="Settings"><GearIcon size={20} /></button>
        </div>
      </div>

      <div className="tab-bar" role="tablist">
        <button className={`tab-bar__tab ${tab === "connect" ? "tab-bar__tab--active" : ""}`} onClick={() => setTab("connect")} role="tab" aria-selected={tab === "connect"}>
          <LinkIcon size={16} /> Connect
        </button>
        <button className={`tab-bar__tab ${tab === "chats" ? "tab-bar__tab--active" : ""}`} onClick={() => setTab("chats")} role="tab" aria-selected={tab === "chats"}>
          <MessageIcon size={16} /> Chats
          {conversations.length > 0 && <span className="tab-bar__badge">{conversations.length}</span>}
        </button>
        <button className={`tab-bar__tab ${tab === "nearby" ? "tab-bar__tab--active" : ""}`} onClick={() => setTab("nearby")} role="tab" aria-selected={tab === "nearby"}>
          <WifiIcon size={16} /> Nearby
          {discoveredPeers.length > 0 && <span className="tab-bar__badge">{discoveredPeers.length}</span>}
        </button>
        <button className={`tab-bar__tab ${tab === "family" ? "tab-bar__tab--active" : ""}`} onClick={() => setTab("family")} role="tab" aria-selected={tab === "family"}>
          <HomeIcon size={16} /> Family
          {family.length > 0 && <span className="tab-bar__badge">{family.length}</span>}
        </button>
      </div>

      <div className="app-content">
        {tab === "connect" ? (
          <ConnectTab
            generatedInvite={generatedInvite} inviteToConnect={inviteToConnect}
            inviteValid={inviteValid} namingMyName={namingMyName} namingTheirName={namingTheirName}
            isConnecting={isConnecting} onGenerateInvite={handleGenerateInvite}
            onCopyInvite={handleCopy} copied={copied}
            setInviteToConnect={setInviteToConnect} onConnect={handleConnect}
            setNamingMyName={setNamingMyName} setNamingTheirName={setNamingTheirName}
            networkSettings={networkSettings} privateMode={privateMode} identity={identity}
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
          <ChatsTab conversations={filtered} onOpenChat={handleOpenChat} onDeleteConversation={handleDeleteConversation} search={search} setSearch={setSearch} />
        )}
      </div>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}

function ConnectTab({ generatedInvite, inviteToConnect, inviteValid, namingMyName, namingTheirName, isConnecting, onGenerateInvite, onCopyInvite, copied, setInviteToConnect, onConnect, setNamingMyName, setNamingTheirName, networkSettings, privateMode, identity }: any) {
  const [generating, setGenerating] = useState(false);
  const [fpCopied, setFpCopied] = useState(false);
  const handleGenerate = async () => { setGenerating(true); try { await onGenerateInvite(); } finally { setGenerating(false); } };

  return (
    <div className="centered-view">
      <div className="invite-section">
        <Card header={{ icon: <PlusIcon size={18} color="var(--color-accent-bright)" />, title: "Host a Connection" }} description="Generate a one-time signed invite for a peer to connect to you securely.">
          {!generatedInvite ? (
            <Button id="generate-invite-btn" onClick={handleGenerate} loading={generating}>Generate Invite Link</Button>
          ) : (
            <div className="invite-output">
              <div className="invite-output__field">
                <span className="invite-output__text">{generatedInvite}</span>
              </div>
              <button className={`btn btn--icon ${copied ? 'btn--icon-copied' : ''}`} onClick={onCopyInvite} id="copy-invite-btn" aria-label="Copy invite">
                {copied ? <span className="copied-pop"><CheckIcon size={18} /></span> : <CopyIcon size={18} />}
              </button>
            </div>
          )}
          {networkSettings?.tor_enabled && !privateMode && generatedInvite && (
            <div className="tor-warning">
              <span>⚠️</span>
              <div><strong className="tor-warning__title">Tor Inbound Warning</strong><p className="tor-warning__text">Tor is enabled for outbound connections, but this invite contains your real IP address.</p></div>
            </div>
          )}
        </Card>

        <Card header={{ icon: <LinkIcon size={18} color="var(--color-success)" />, iconVariant: "success" as const, title: "Join a Connection" }} description="Paste an invite link from a trusted peer to connect.">
          <div className="flex-row">
            <Input id="invite-input" placeholder="m2m://..." value={inviteToConnect} onChange={e => setInviteToConnect(e.target.value)} mono clearable onClear={() => setInviteToConnect("")} />
            <Button id="connect-btn" onClick={onConnect} disabled={isConnecting || !inviteToConnect} loading={isConnecting} size="sm">Connect</Button>
          </div>
          {inviteValid && (
            <div className="naming-panel">
              <div className="naming-panel__valid"><CheckIcon size={16} /> Valid Invite Found</div>
              <label>Your Name <Input placeholder="How they will see you" value={namingMyName} onChange={e => setNamingMyName(e.target.value)} compact /></label>
              <label>Their Name <Input placeholder="How you want to see them" value={namingTheirName} onChange={e => setNamingTheirName(e.target.value)} compact /></label>
            </div>
          )}
        </Card>

        <div className="divider" />

        <div className="fingerprint-box" id="fingerprint-display">
          <span className="fingerprint-label">Your Identity Fingerprint</span>
          <span className="fingerprint-value-row">
            {identity?.fingerprint}
            <button className="btn btn--ghost btn--icon-sm" onClick={() => {
              if (identity?.fingerprint) {
                navigator.clipboard.writeText(identity.fingerprint);
                setFpCopied(true);
                setTimeout(() => setFpCopied(false), 2000);
              }
            }} aria-label="Copy">
              {fpCopied ? <span className="copied-pop"><CheckIcon size={14} /></span> : <CopyIcon size={14} />}
            </button>
          </span>
        </div>
      </div>
    </div>
  );
}

function ChatsTab({ conversations, onOpenChat, onDeleteConversation, search, setSearch }: any) {
  return (
    <div className="conv-list">
      {conversations.length > 0 && (
        <div className="conv-search">
          <Input placeholder="Search conversations…" value={search} onChange={e => setSearch(e.target.value)} icon={<SearchIcon size={16} />} clearable onClear={() => setSearch("")} />
        </div>
      )}

      {conversations.length === 0 ? (
        <div className="conv-empty">
          <MessageIcon size={48} color="var(--color-text-muted)" />
          <span style={{ fontSize: 'var(--text-lg)', fontWeight: 600, color: 'var(--color-text-primary)' }}>
            {search ? "No conversations found" : "No conversations yet"}
          </span>
          <span style={{ maxWidth: '320px', textAlign: 'center', lineHeight: 1.6 }}>
            {search 
              ? "Try adjusting your search terms or clear the filter." 
              : "Generate an invite link to host a connection, or paste an invite from a peer to join."}
          </span>
          {!search && (
            <Button onClick={() => setTab("connect")} icon={<PlusIcon size={18} />} style={{ marginTop: 'var(--space-md)' }}>
              Get Started
            </Button>
          )}
        </div>
      ) : (
        conversations.map((c: any) => (
          <div key={c.id} className="conv-item" onClick={() => onOpenChat(c)} role="button" tabIndex={0} onKeyDown={e => e.key === "Enter" && onOpenChat(c)}>
            <div className={`conv-avatar ${c.is_online ? 'conv-avatar--online' : 'conv-avatar--offline'}`} style={{
              background: `linear-gradient(135deg, ${hashToColor(c.peer_key_hex)}, ${hashToColor(c.peer_key_hex.slice(16))})`,
            }}>
              {(c.display_name || c.peer_display_name || c.peer_key_hex).charAt(0).toUpperCase()}
            </div>
            <div className="conv-body">
              <div className="conv-top">
                <span className="conv-name">{c.display_name || c.peer_display_name || "Unknown Peer"}</span>
                {c.last_message_at && <span className="conv-time">{formatTime(c.last_message_at)}</span>}
              </div>
              <span className="conv-preview">{c.last_message_preview || "No messages yet."}</span>
            </div>
            <div className="conv-status">{c.is_online ? <OnlineDot /> : <OfflineDot />}</div>
            <div className="conv-actions">
              <button className="btn btn--icon btn--icon-sm"
                onClick={e => { e.stopPropagation(); invoke("delete_conversation_cmd", { conversationId: c.id }).then(() => onDeleteConversation(c.id)).catch(console.error); }}
                aria-label="Delete">
                <TrashIcon size={16} />
              </button>
            </div>
          </div>
        ))
      )}
    </div>
  );
}

function NearbyTab({ discoveryConfig, discoveredPeers, onConnect, onRefresh, onOpenSettings, onOpenChat }: any) {
  const [connecting, setConnecting] = useState<string | null>(null);

  const handleConnectPeer = async (address: string) => {
    setConnecting(address);
    try {
      const info = await onConnect(address);
      if (info?.peer_key_hex && onOpenChat) {
        onOpenChat({
          peer_key_hex: info.peer_key_hex,
          is_online: true,
          retention_policy: "none",
          display_name: null,
          peer_display_name: null,
          id: info.peer_key_hex,
        });
      }
    } catch {
      // toast already shown by handler
    } finally {
      setConnecting(null);
    }
  };

  // Discovery not active
  if (!discoveryConfig?.lan_enabled && !discoveryConfig?.dht_enabled) {
    return (
      <div className="centered-view">
        <div className="nearby-empty">
          <span style={{ fontSize: 'var(--text-lg)', fontWeight: 600, color: 'var(--color-text-primary)' }}>
            Discovery Not Active
          </span>
          <span style={{ maxWidth: '320px', textAlign: 'center', lineHeight: 1.6, color: 'var(--color-text-muted)' }}>
            Enable LAN or DHT discovery in Settings to find nearby peers.
            Both are <strong>OFF by default</strong> — privacy first.
          </span>
          <Button variant="secondary" size="sm" onClick={onOpenSettings} style={{ marginTop: 'var(--space-md)' }}>
            <GearIcon size={16} /> Open Settings
          </Button>
        </div>
      </div>
    );
  }

  // No peers found
  if (discoveredPeers.length === 0) {
    return (
      <div className="centered-view">
        <div className="nearby-empty">
          <WifiIcon size={48} color="var(--color-text-muted)" />
          <span style={{ fontSize: 'var(--text-lg)', fontWeight: 600, color: 'var(--color-text-primary)' }}>
            No Peers Found
          </span>
          <span style={{ maxWidth: '320px', textAlign: 'center', lineHeight: 1.6, color: 'var(--color-text-muted)' }}>
            {discoveryConfig?.lan_enabled
              ? "No LAN peers detected. Make sure other M2M users are on the same network with LAN discovery enabled."
              : ""}
            {discoveryConfig?.lan_enabled && discoveryConfig?.dht_enabled ? " " : ""}
            {discoveryConfig?.dht_enabled
              ? "No DHT peers found. They may be offline or behind a symmetric NAT."
              : ""}
          </span>
          <Button variant="secondary" size="xs" onClick={onRefresh} style={{ marginTop: 'var(--space-md)' }}>
            Refresh
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="conv-list">
      <div style={{ display: 'flex', justifyContent: 'flex-end', padding: 'var(--space-xs) var(--space-md)', gap: 'var(--space-xs)' }}>
        <Button variant="secondary" size="xs" onClick={onRefresh}>Refresh</Button>
      </div>
      {discoveredPeers.map((peer: any, idx: number) => (
        <div key={`${peer.method}-${peer.id_hex}-${idx}`} className="conv-item" role="listitem">
          <div className="conv-avatar conv-avatar--online" style={{
            background: `linear-gradient(135deg, #22c55e, #16a34a)`,
          }}>
            <WifiIcon size={18} color="white" />
          </div>
          <div className="conv-body">
            <div className="conv-top">
              <span className="conv-name">
                {peer.method === "lan" ? "LAN Peer" : "DHT Peer"}
              </span>
              <span className="conv-time">{formatTime(peer.last_seen)}</span>
            </div>
            <div className="conv-preview">
              {peer.address}
              <span className={`badge badge--${peer.method === "lan" ? "info" : "warning"}`} style={{ marginLeft: 'var(--space-xs)', fontSize: '0.7rem' }}>
                {peer.method === "lan" ? "LAN" : "DHT"}
              </span>
            </div>
            <div className="conv-preview" style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', fontFamily: 'var(--font-mono)' }}>
              {peer.id_hex.slice(0, 16)}...
            </div>
          </div>
          <div className="conv-status" style={{ gap: 'var(--space-xxs)' }}>
            <Button
              size="xs"
              onClick={() => handleConnectPeer(peer.address)}
              disabled={connecting === peer.address}
              loading={connecting === peer.address}
            >
              Connect
            </Button>
          </div>
        </div>
      ))}
    </div>
  );
}

function hashToColor(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) hash = str.charCodeAt(i) + ((hash << 5) - hash);
  return `hsl(${Math.abs(hash) % 360}, 55%, 48%)`;
}

function formatTime(ts: number): string {
  const d = Math.floor(Date.now() / 1000) - ts;
  if (d < 60) return "now";
  if (d < 3600) return `${Math.floor(d / 60)}m ago`;
  if (d < 86400) return `${Math.floor(d / 3600)}h ago`;
  if (d < 604800) return `${Math.floor(d / 86400)}d ago`;
  return new Date(ts * 1000).toLocaleDateString();
}
