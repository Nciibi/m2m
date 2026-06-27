import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Button,
  Input,
  Card,
  Badge,
  ToastContainer,
} from "../components/ui";
import type {
  Toast,
  ConversationEntry,
  IdentityInfo,
  NetworkSettings,
} from "../types";

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
  identity,
  toasts,
  removeToast,
  generatedInvite,
  inviteToConnect,
  inviteValid,
  namingMyName,
  namingTheirName,
  isConnecting,
  onGenerateInvite,
  onCopyInvite,
  setInviteToConnect,
  onConnect,
  setNamingMyName,
  setNamingTheirName,
  onOpenChat,
  onOpenSettings,
  onDeleteConversation,
  conversations,
  networkSettings,
  privateMode,
}: Props) {
  const [hubTab, setHubTab] = useState<"connect" | "chats">("connect");
  const [copied, setCopied] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  // Copy with feedback
  const handleCopy = () => {
    onCopyInvite();
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Filter conversations by search
  const filteredConversations = conversations.filter((c) => {
    if (!searchQuery) return true;
    const q = searchQuery.toLowerCase();
    return (
      (c.display_name || "").toLowerCase().includes(q) ||
      (c.peer_display_name || "").toLowerCase().includes(q) ||
      (c.last_message_preview || "").toLowerCase().includes(q) ||
      c.peer_key_hex.toLowerCase().includes(q)
    );
  });

  return (
    <div className="app-container">
      {/* Header */}
      <div className="header">
        <h1>
          <span>🛡️</span> M2M
        </h1>
        <div className="header-actions">
          <Badge variant="default" compact>
            Offline
          </Badge>
          <Button
            variant="icon"
            onClick={onOpenSettings}
            id="settings-btn"
            aria-label="Settings"
          >
            ⚙️
          </Button>
        </div>
      </div>

      {/* Tabs */}
      <div className="hub-tabs" role="tablist">
        <button
          className={`hub-tab ${hubTab === "connect" ? "active" : ""}`}
          onClick={() => setHubTab("connect")}
          role="tab"
          aria-selected={hubTab === "connect"}
        >
          🔌 Connect
        </button>
        <button
          className={`hub-tab ${hubTab === "chats" ? "active" : ""}`}
          onClick={() => setHubTab("chats")}
          role="tab"
          aria-selected={hubTab === "chats"}
        >
          💬 Chats
          {conversations.length > 0 && (
            <span className="tab-badge">{conversations.length}</span>
          )}
        </button>
      </div>

      {/* Tab content */}
      <div className="content-area hub-tab-content">
        {hubTab === "connect" ? (
          <ConnectTab
            generatedInvite={generatedInvite}
            inviteToConnect={inviteToConnect}
            inviteValid={inviteValid}
            namingMyName={namingMyName}
            namingTheirName={namingTheirName}
            isConnecting={isConnecting}
            onGenerateInvite={onGenerateInvite}
            onCopyInvite={handleCopy}
            copied={copied}
            setInviteToConnect={setInviteToConnect}
            onConnect={onConnect}
            setNamingMyName={setNamingMyName}
            setNamingTheirName={setNamingTheirName}
            networkSettings={networkSettings}
            privateMode={privateMode}
            identity={identity}
          />
        ) : (
          <ChatsTab
            conversations={filteredConversations}
            onOpenChat={onOpenChat}
            onDeleteConversation={onDeleteConversation}
            searchQuery={searchQuery}
            setSearchQuery={setSearchQuery}
          />
        )}
      </div>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}

// ─── Connect Tab ───

interface ConnectTabProps {
  generatedInvite: string;
  inviteToConnect: string;
  inviteValid: boolean;
  namingMyName: string;
  namingTheirName: string;
  isConnecting: boolean;
  onGenerateInvite: () => Promise<void>;
  onCopyInvite: () => void;
  copied: boolean;
  setInviteToConnect: (v: string) => void;
  onConnect: () => Promise<void>;
  setNamingMyName: (v: string) => void;
  setNamingTheirName: (v: string) => void;
  networkSettings: NetworkSettings | null;
  privateMode: boolean;
  identity: IdentityInfo | null;
}

function ConnectTab({
  generatedInvite,
  inviteToConnect,
  inviteValid,
  namingMyName,
  namingTheirName,
  isConnecting,
  onGenerateInvite,
  onCopyInvite,
  copied,
  setInviteToConnect,
  onConnect,
  setNamingMyName,
  setNamingTheirName,
  networkSettings,
  privateMode,
  identity,
}: ConnectTabProps) {
  const [generating, setGenerating] = useState(false);

  const handleGenerate = async () => {
    setGenerating(true);
    try {
      await onGenerateInvite();
    } finally {
      setGenerating(false);
    }
  };

  return (
    <div className="centered-view">
      <div className="invite-section">
        {/* Host Card */}
        <Card
          header={{ icon: "➕", title: "Host a Connection" }}
          description="Generate a one-time signed invite for a peer to connect to you securely."
        >
          {!generatedInvite ? (
            <Button
              id="generate-invite-btn"
              onClick={handleGenerate}
              loading={generating}
            >
              Generate Invite Link
            </Button>
          ) : (
            <div className="invite-output">
              <input
                readOnly
                value={generatedInvite}
                id="invite-output"
                style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-base)" }}
              />
              <Button
                variant="icon"
                onClick={onCopyInvite}
                id="copy-invite-btn"
                aria-label="Copy invite to clipboard"
              >
                {copied ? "✅" : "📋"}
              </Button>
            </div>
          )}

          {/* Tor inbound warning */}
          {networkSettings?.tor_enabled && !privateMode && generatedInvite && (
            <div className="tor-warning-banner" id="tor-inbound-warning">
              <div className="tor-warning-icon">⚠️</div>
              <div className="tor-warning-content">
                <strong>Tor Inbound Warning</strong>
                <p>
                  Tor is enabled for <em>outbound</em> connections, but this invite
                  contains your real IP address. Inbound connections will bypass Tor
                  and reveal your location.
                </p>
              </div>
            </div>
          )}
        </Card>

        {/* Join Card */}
        <Card
          header={{ icon: "🔗", title: "Join a Connection", iconVariant: "success" }}
          description="Paste an invite link from a trusted peer to connect."
        >
          <div className="flex-row">
            <Input
              id="invite-input"
              placeholder="m2m://..."
              value={inviteToConnect}
              onChange={(e) => setInviteToConnect(e.target.value)}
              mono
              clearable
              onClear={() => setInviteToConnect("")}
            />
            <Button
              id="connect-btn"
              onClick={onConnect}
              disabled={isConnecting || !inviteToConnect}
              loading={isConnecting}
              compact
            >
              Connect
            </Button>
          </div>

          {inviteValid && (
            <div className="naming-panel">
              <div className="valid-badge">✅ Valid Invite Found</div>
              <label>
                Your Display Name
                <Input
                  placeholder="How they will see you"
                  value={namingMyName}
                  onChange={(e) => setNamingMyName(e.target.value)}
                  compact
                />
              </label>
              <label>
                Their Display Name
                <Input
                  placeholder="How you want to see them"
                  value={namingTheirName}
                  onChange={(e) => setNamingTheirName(e.target.value)}
                  compact
                />
              </label>
            </div>
          )}
        </Card>

        <div className="section-divider" />

        {/* Fingerprint */}
        <div className="fingerprint-box" id="fingerprint-display">
          <span className="fingerprint-label">Your Identity Fingerprint</span>
          <span style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: 8 }}>
            {identity?.fingerprint}
            <Button
              variant="ghost"
              compact
              onClick={() => {
                if (identity?.fingerprint) {
                  navigator.clipboard.writeText(identity.fingerprint);
                }
              }}
              aria-label="Copy fingerprint"
              style={{ fontSize: "var(--text-sm)", padding: "2px 6px" }}
            >
              📋
            </Button>
          </span>
        </div>
      </div>
    </div>
  );
}

// ─── Chats Tab ───

interface ChatsTabProps {
  conversations: ConversationEntry[];
  onOpenChat: (conv: ConversationEntry) => void;
  onDeleteConversation: (id: string) => void;
  searchQuery: string;
  setSearchQuery: (v: string) => void;
}

function ChatsTab({
  conversations,
  onOpenChat,
  onDeleteConversation,
  searchQuery,
  setSearchQuery,
}: ChatsTabProps) {
  return (
    <div className="conversation-list">
      {/* Search bar */}
      {conversations.length > 0 && (
        <div style={{ padding: "8px 0 12px" }}>
          <Input
            placeholder="Search conversations…"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            icon="🔍"
            clearable
            onClear={() => setSearchQuery("")}
          />
        </div>
      )}

      {conversations.length === 0 ? (
        <div className="conversation-list-empty">
          <span className="empty-icon" aria-hidden="true">
            {searchQuery ? "🔍" : "📭"}
          </span>
          {searchQuery
            ? "No conversations match your search."
            : "No conversations yet. Connect to a peer to start chatting!"}
        </div>
      ) : (
        conversations.map((c) => (
          <div
            key={c.id}
            className="conversation-item"
            onClick={() => onOpenChat(c)}
            role="button"
            tabIndex={0}
            onKeyDown={(e) => {
              if (e.key === "Enter") onOpenChat(c);
            }}
          >
            {/* Avatar with dynamic color */}
            <div
              className={`conv-avatar ${c.is_online ? "online" : ""}`}
              style={{
                background: `linear-gradient(135deg, ${hashToColor(c.peer_key_hex)}, ${hashToColor(c.peer_key_hex.slice(16))})`,
                color: "white",
                border: c.is_online
                  ? "2px solid var(--color-success)"
                  : "2px solid rgba(255,255,255,0.1)",
              }}
            >
              {(c.display_name || c.peer_display_name || c.peer_key_hex).charAt(0).toUpperCase()}
            </div>

            {/* Content */}
            <div className="conv-body">
              <div className="conv-top-row">
                <span className="conv-name">
                  {c.display_name || c.peer_display_name || "Unknown Peer"}
                </span>
                {c.last_message_at && (
                  <span className="conv-time">
                    {formatRelativeTime(c.last_message_at)}
                  </span>
                )}
              </div>
              <div className="conv-preview">
                {c.last_message_preview || "No messages yet."}
              </div>
              {c.retention_policy !== "none" && (
                <div className="conv-retention-badge">
                  ⏳ Policy: {c.retention_policy}
                </div>
              )}
            </div>

            {/* Status dot */}
            <div
              className={`conv-status-dot ${c.is_online ? "online" : "offline"}`}
              title={c.is_online ? "Online" : "Offline"}
            />

            {/* Actions */}
            <div className="conv-actions">
              <Button
                variant="danger"
                compact
                onClick={(e) => {
                  e.stopPropagation();
                  invoke("delete_conversation_cmd", {
                    conversationId: c.id,
                  })
                    .then(() => onDeleteConversation(c.id))
                    .catch(console.error);
                }}
                aria-label={`Delete conversation with ${c.display_name || c.peer_key_hex}`}
              >
                Delete
              </Button>
            </div>
          </div>
        ))
      )}
    </div>
  );
}

// ─── Helpers ───

/** Hash a string to an HSL color for avatars. */
function hashToColor(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = str.charCodeAt(i) + ((hash << 5) - hash);
  }
  const hue = Math.abs(hash) % 360;
  return `hsl(${hue}, 60%, 50%)`;
}

/** Format a unix timestamp as a relative time string. */
function formatRelativeTime(ts: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - ts;
  if (diff < 60) return "now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  return new Date(ts * 1000).toLocaleDateString();
}
