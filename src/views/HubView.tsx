import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, Input, Card, Badge, ToastContainer } from "../components/ui";
import {
  ShieldIcon, GearIcon, PlusIcon, LinkIcon, CopyIcon, CheckIcon,
  SearchIcon, MessageIcon, TrashIcon, OnlineDot, OfflineDot,
} from "../components/ui/Icons";
import type { Toast, ConversationEntry, IdentityInfo, NetworkSettings } from "../types";

interface Props {
  identity: IdentityInfo | null;
  toasts: Toast[];
  removeToast: (id: string) => void;
  generatedInvite: string;
  inviteToConnect: string;
  inviteValid: boolean;
  namingMyName: string;
  namingTheirName: string;
  isConnecting: boolean;
  onGenerateInvite: () => Promise<void>;
  onCopyInvite: () => void;
  setInviteToConnect: (v: string) => void;
  onConnect: () => Promise<void>;
  setNamingMyName: (v: string) => void;
  setNamingTheirName: (v: string) => void;
  onOpenChat: (conv: ConversationEntry) => void;
  onOpenSettings: () => void;
  onDeleteConversation: (id: string) => void;
  conversations: ConversationEntry[];
  networkSettings: NetworkSettings | null;
  privateMode: boolean;
}

export default function HubView({
  identity, toasts, removeToast, generatedInvite, inviteToConnect,
  inviteValid, namingMyName, namingTheirName, isConnecting,
  onGenerateInvite, onCopyInvite, setInviteToConnect, onConnect,
  setNamingMyName, setNamingTheirName, onOpenChat, onOpenSettings,
  onDeleteConversation, conversations, networkSettings, privateMode,
}: Props) {
  const [tab, setTab] = useState<"connect" | "chats">("connect");
  const [copied, setCopied] = useState(false);
  const [search, setSearch] = useState("");

  const handleCopy = () => { onCopyInvite(); setCopied(true); setTimeout(() => setCopied(false), 2000); };

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
          <span style={{ display: "inline-flex", width: 32, height: 32, borderRadius: "var(--radius-sm)", background: "var(--color-accent-gradient)", alignItems: "center", justifyContent: "center", boxShadow: "var(--shadow-accent)" }}>
            <ShieldIcon size={18} color="white" />
          </span>
          M2M
        </h1>
        <div className="app-header__actions">
          <Badge variant="default" compact><OfflineDot /> Offline</Badge>
          <button className="btn btn--icon" onClick={onOpenSettings} id="settings-btn" aria-label="Settings"><GearIcon size={20} /></button>
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
      </div>

      <div className="app-content">
        {tab === "connect" ? (
          <ConnectTab
            generatedInvite={generatedInvite} inviteToConnect={inviteToConnect}
            inviteValid={inviteValid} namingMyName={namingMyName} namingTheirName={namingTheirName}
            isConnecting={isConnecting} onGenerateInvite={onGenerateInvite}
            onCopyInvite={handleCopy} copied={copied}
            setInviteToConnect={setInviteToConnect} onConnect={onConnect}
            setNamingMyName={setNamingMyName} setNamingTheirName={setNamingTheirName}
            networkSettings={networkSettings} privateMode={privateMode} identity={identity}
          />
        ) : (
          <ChatsTab conversations={filtered} onOpenChat={onOpenChat} onDeleteConversation={onDeleteConversation} search={search} setSearch={setSearch} />
        )}
      </div>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}

function ConnectTab({ generatedInvite, inviteToConnect, inviteValid, namingMyName, namingTheirName, isConnecting, onGenerateInvite, onCopyInvite, copied, setInviteToConnect, onConnect, setNamingMyName, setNamingTheirName, networkSettings, privateMode, identity }: any) {
  const [generating, setGenerating] = useState(false);
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
              <button className={`btn btn--icon`} onClick={onCopyInvite} id="copy-invite-btn" aria-label="Copy invite" style={{ borderColor: copied ? "rgba(16,185,129,0.3)" : undefined, background: copied ? "var(--color-success-bg)" : undefined }}>
                {copied ? <CheckIcon size={18} color="var(--color-success)" /> : <CopyIcon size={18} />}
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
          <span style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: "var(--space-xs)" }}>
            {identity?.fingerprint}
            <button className="btn btn--ghost" onClick={() => identity?.fingerprint && navigator.clipboard.writeText(identity.fingerprint)} aria-label="Copy" style={{ padding: "4px 8px", minWidth: "auto", minHeight: "auto", borderRadius: "var(--radius-xs)" }}>
              <CopyIcon size={14} />
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
        <div style={{ paddingBottom: "var(--space-sm)" }}>
          <Input placeholder="Search conversations…" value={search} onChange={e => setSearch(e.target.value)} icon={<SearchIcon size={16} />} clearable onClear={() => setSearch("")} />
        </div>
      )}

      {conversations.length === 0 ? (
        <div className="conv-empty">
          <MessageIcon size={48} color="var(--color-text-muted)" />
          <span>{search ? "No conversations match your search." : "No conversations yet. Connect to a peer to start chatting!"}</span>
        </div>
      ) : (
        conversations.map((c: any) => (
          <div key={c.id} className="conv-item" onClick={() => onOpenChat(c)} role="button" tabIndex={0} onKeyDown={e => e.key === "Enter" && onOpenChat(c)}>
            <div className="conv-avatar" style={{
              background: `linear-gradient(135deg, ${hashToColor(c.peer_key_hex)}, ${hashToColor(c.peer_key_hex.slice(16))})`,
              border: c.is_online ? "2px solid var(--color-success)" : "2px solid rgba(255,255,255,0.08)",
              boxShadow: c.is_online ? "0 0 15px var(--color-success-glow)" : "0 4px 10px rgba(0,0,0,0.2)",
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
              <button className="btn btn--icon" style={{ padding: 6, minWidth: "auto", minHeight: "auto", borderRadius: "var(--radius-sm)" }}
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
