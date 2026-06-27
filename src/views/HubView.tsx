import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, Input, Card, Badge, ToastContainer } from "../components/ui";
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

  const handleCopy = () => {
    onCopyInvite();
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

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
      {/* Premium Header */}
      <div className="header">
        <h1>
          <span
            style={{
              display: "inline-flex",
              width: 32,
              height: 32,
              borderRadius: "var(--radius-sm)",
              background: "var(--color-accent-gradient)",
              alignItems: "center",
              justifyContent: "center",
              fontSize: "1rem",
              boxShadow: "var(--shadow-accent)",
            }}
          >
            🛡️
          </span>
          M2M
        </h1>
        <div className="header-actions">
          <Badge variant="default" compact>
            <span
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 6,
              }}
            >
              <span
                style={{
                  width: 6,
                  height: 6,
                  borderRadius: "50%",
                  background: "var(--color-text-muted)",
                  display: "inline-block",
                }}
              />
              Offline
            </span>
          </Badge>
          <button
            onClick={onOpenSettings}
            className="icon-btn"
            id="settings-btn"
            aria-label="Settings"
            title="Settings"
          >
            ⚙️
          </button>
        </div>
      </div>

      {/* Premium Tab Bar */}
      <div
        style={{
          display: "flex",
          gap: 0,
          padding: "0 var(--space-2xl)",
          borderBottom: "1px solid var(--color-border-default)",
          background: "linear-gradient(to bottom, rgba(0,0,0,0.15), transparent)",
          flexShrink: 0,
        }}
        role="tablist"
      >
        <button
          className={`hub-tab ${hubTab === "connect" ? "active" : ""}`}
          onClick={() => setHubTab("connect")}
          role="tab"
          aria-selected={hubTab === "connect"}
          style={{
            flex: 1,
            padding: "var(--space-md) var(--space-lg)",
            background: "transparent",
            border: "none",
            borderBottom: "3px solid",
            borderBottomColor:
              hubTab === "connect" ? "var(--color-accent)" : "transparent",
            color:
              hubTab === "connect"
                ? "var(--color-text-accent)"
                : "var(--color-text-muted)",
            fontSize: "var(--text-md)",
            fontWeight: 600,
            cursor: "pointer",
            transition: "var(--transition-base)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            gap: "var(--space-xs)",
            fontFamily: "inherit",
            position: "relative",
          }}
          onMouseEnter={(e) => {
            if (hubTab !== "connect") {
              e.currentTarget.style.color = "var(--color-text-secondary)";
            }
          }}
          onMouseLeave={(e) => {
            if (hubTab !== "connect") {
              e.currentTarget.style.color = "var(--color-text-muted)";
            }
          }}
        >
          <span>🔌</span> Connect
        </button>
        <button
          className={`hub-tab ${hubTab === "chats" ? "active" : ""}`}
          onClick={() => setHubTab("chats")}
          role="tab"
          aria-selected={hubTab === "chats"}
          style={{
            flex: 1,
            padding: "var(--space-md) var(--space-lg)",
            background: "transparent",
            border: "none",
            borderBottom: "3px solid",
            borderBottomColor:
              hubTab === "chats" ? "var(--color-accent)" : "transparent",
            color:
              hubTab === "chats"
                ? "var(--color-text-accent)"
                : "var(--color-text-muted)",
            fontSize: "var(--text-md)",
            fontWeight: 600,
            cursor: "pointer",
            transition: "var(--transition-base)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            gap: "var(--space-xs)",
            fontFamily: "inherit",
          }}
          onMouseEnter={(e) => {
            if (hubTab !== "chats") {
              e.currentTarget.style.color = "var(--color-text-secondary)";
            }
          }}
          onMouseLeave={(e) => {
            if (hubTab !== "chats") {
              e.currentTarget.style.color = "var(--color-text-muted)";
            }
          }}
        >
          <span>💬</span> Chats
          {conversations.length > 0 && (
            <span
              style={{
                background: "var(--color-accent-gradient)",
                color: "white",
                fontSize: "var(--text-xs)",
                padding: "2px 8px",
                borderRadius: "var(--radius-full)",
                fontWeight: 700,
                minWidth: 22,
                textAlign: "center",
                boxShadow: "var(--shadow-accent)",
              }}
            >
              {conversations.length}
            </span>
          )}
        </button>
      </div>

      {/* Tab content */}
      <div className="content-area">
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
      <div
        className="invite-section"
        style={{
          width: "100%",
          maxWidth: 480,
          display: "flex",
          flexDirection: "column",
          gap: "var(--space-md)",
        }}
      >
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
              icon="✨"
            >
              Generate Invite Link
            </Button>
          ) : (
            <div
              className="invite-output"
              style={{
                display: "flex",
                gap: "var(--space-xs)",
                alignItems: "stretch",
              }}
            >
              <div
                style={{
                  flex: 1,
                  display: "flex",
                  alignItems: "center",
                  background: "rgba(0,0,0,0.3)",
                  border: "1px solid var(--color-border-accent)",
                  borderRadius: "var(--radius-md)",
                  padding: "var(--space-sm) var(--space-md)",
                  fontFamily: "var(--font-mono)",
                  fontSize: "var(--text-base)",
                  color: "var(--color-text-accent)",
                  overflow: "hidden",
                  minHeight: 44,
                }}
              >
                <span
                  style={{
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {generatedInvite}
                </span>
              </div>
              <button
                onClick={onCopyInvite}
                className="icon-btn"
                id="copy-invite-btn"
                aria-label="Copy invite to clipboard"
                title="Copy invite"
                style={{
                  minWidth: 44,
                  minHeight: 44,
                  borderColor: copied
                    ? "rgba(16,185,129,0.3)"
                    : "var(--color-border-default)",
                  background: copied
                    ? "var(--color-success-bg)"
                    : "transparent",
                }}
              >
                {copied ? "✅" : "📋"}
              </button>
            </div>
          )}

          {/* Tor inbound warning */}
          {networkSettings?.tor_enabled && !privateMode && generatedInvite && (
            <div
              id="tor-inbound-warning"
              style={{
                display: "flex",
                gap: "var(--space-sm)",
                padding: "var(--space-md)",
                marginTop: "var(--space-xs)",
                background: "var(--color-warning-bg)",
                border: "1px solid rgba(245,158,11,0.2)",
                borderRadius: "var(--radius-md)",
                alignItems: "flex-start",
                animation: "msgSlide 300ms var(--ease-out-expo)",
              }}
            >
              <span
                style={{
                  fontSize: "1.2rem",
                  flexShrink: 0,
                  lineHeight: 1.4,
                }}
              >
                ⚠️
              </span>
              <div style={{ flex: 1 }}>
                <strong
                  style={{
                    display: "block",
                    fontSize: "var(--text-sm)",
                    color: "var(--color-warning)",
                    marginBottom: "var(--space-xxs)",
                  }}
                >
                  Tor Inbound Warning
                </strong>
                <p
                  style={{
                    fontSize: "var(--text-sm)",
                    color: "var(--color-text-secondary)",
                    lineHeight: 1.5,
                    margin: 0,
                  }}
                >
                  Tor is enabled for <em>outbound</em> connections, but this
                  invite contains your real IP address. Inbound connections will
                  bypass Tor and reveal your location.
                </p>
              </div>
            </div>
          )}
        </Card>

        {/* Join Card */}
        <Card
          header={{
            icon: "🔗",
            title: "Join a Connection",
            iconVariant: "success",
          }}
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
            <div
              className="naming-panel"
              style={{
                marginTop: "var(--space-sm)",
                padding: "var(--space-md)",
                background: "var(--color-bg-elevated)",
                borderRadius: "var(--radius-md)",
                border: "1px solid var(--color-border-accent)",
                display: "flex",
                flexDirection: "column",
                gap: "var(--space-sm)",
                animation: "expandDown 300ms var(--ease-out-expo)",
                overflow: "hidden",
              }}
            >
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "var(--space-xs)",
                  fontSize: "var(--text-sm)",
                  color: "var(--color-success)",
                  marginBottom: "var(--space-xxs)",
                }}
              >
                <span>✅</span>
                <span style={{ fontWeight: 600 }}>Valid Invite Found</span>
              </div>
              <label
                style={{
                  fontSize: "var(--text-sm)",
                  color: "var(--color-text-secondary)",
                  display: "flex",
                  flexDirection: "column",
                  gap: "var(--space-xxs)",
                }}
              >
                Your Display Name
                <Input
                  placeholder="How they will see you"
                  value={namingMyName}
                  onChange={(e) => setNamingMyName(e.target.value)}
                  compact
                />
              </label>
              <label
                style={{
                  fontSize: "var(--text-sm)",
                  color: "var(--color-text-secondary)",
                  display: "flex",
                  flexDirection: "column",
                  gap: "var(--space-xxs)",
                }}
              >
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
          <span className="fingerprint-label">
            Your Identity Fingerprint
          </span>
          <span
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "var(--space-xs)",
            }}
          >
            {identity?.fingerprint}
            <button
              className="icon-btn"
              onClick={() => {
                if (identity?.fingerprint) {
                  navigator.clipboard.writeText(identity.fingerprint);
                }
              }}
              aria-label="Copy fingerprint"
              title="Copy fingerprint"
              style={{
                fontSize: "var(--text-sm)",
                padding: "4px 8px",
                minWidth: "auto",
                minHeight: "auto",
                border: "1px solid var(--color-border-default)",
                background: "transparent",
                borderRadius: "var(--radius-xs)",
                color: "var(--color-text-muted)",
                cursor: "pointer",
                fontFamily: "inherit",
              }}
            >
              📋
            </button>
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
    <div
      className="conversation-list"
      style={{
        padding: "var(--space-md) var(--space-xl)",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-xs)",
        overflowY: "auto",
        flex: 1,
        scrollbarWidth: "thin",
        scrollbarColor: "rgba(255,255,255,0.06) transparent",
      }}
    >
      {/* Search bar */}
      {conversations.length > 0 && (
        <div style={{ padding: "0 0 var(--space-sm)" }}>
          <Input
            placeholder="Search conversations…"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            icon={<span>🔍</span>}
            clearable
            onClear={() => setSearchQuery("")}
          />
        </div>
      )}

      {conversations.length === 0 ? (
        <div
          style={{
            textAlign: "center",
            padding: "var(--space-4xl) var(--space-xl)",
            color: "var(--color-text-muted)",
            fontSize: "var(--text-md)",
            fontWeight: 500,
            flex: 1,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: "var(--space-md)",
          }}
        >
          <span
            style={{
              fontSize: "3rem",
              display: "block",
              opacity: 0.5,
            }}
          >
            {searchQuery ? "🔍" : "📭"}
          </span>
          <span>
            {searchQuery
              ? "No conversations match your search."
              : "No conversations yet. Connect to a peer to start chatting!"}
          </span>
        </div>
      ) : (
        conversations.map((c) => (
          <div
            key={c.id}
            onClick={() => onOpenChat(c)}
            role="button"
            tabIndex={0}
            onKeyDown={(e) => {
              if (e.key === "Enter") onOpenChat(c);
            }}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "var(--space-md)",
              padding: "var(--space-md) var(--space-lg)",
              borderRadius: "var(--radius-lg)",
              cursor: "pointer",
              transition: "var(--transition-base)",
              border: "1px solid rgba(255,255,255,0.02)",
              background: "rgba(255,255,255,0.02)",
              position: "relative",
            }}
            onMouseEnter={(e) => {
              const t = e.currentTarget;
              t.style.background = "rgba(255,255,255,0.05)";
              t.style.borderColor = "rgba(255,255,255,0.08)";
              t.style.transform = "translateY(-1px)";
              t.style.boxShadow = "var(--shadow-sm)";
            }}
            onMouseLeave={(e) => {
              const t = e.currentTarget;
              t.style.background = "rgba(255,255,255,0.02)";
              t.style.borderColor = "rgba(255,255,255,0.02)";
              t.style.transform = "";
              t.style.boxShadow = "none";
            }}
          >
            {/* Dynamic color avatar */}
            <div
              style={{
                width: 48,
                height: 48,
                borderRadius: 14,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                fontSize: "var(--text-xl)",
                fontWeight: 700,
                flexShrink: 0,
                color: "white",
                border: c.is_online
                  ? "2px solid var(--color-success)"
                  : "2px solid rgba(255,255,255,0.08)",
                boxShadow: c.is_online
                  ? "0 0 15px var(--color-success-glow)"
                  : "0 4px 10px rgba(0,0,0,0.2)",
                textTransform: "uppercase",
                background: `linear-gradient(135deg, ${hashToColor(c.peer_key_hex)}, ${hashToColor(c.peer_key_hex.slice(16))})`,
              }}
            >
              {(c.display_name || c.peer_display_name || c.peer_key_hex)
                .charAt(0)
                .toUpperCase()}
            </div>

            {/* Content */}
            <div
              style={{
                flex: 1,
                minWidth: 0,
                display: "flex",
                flexDirection: "column",
                gap: 2,
              }}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                }}
              >
                <span
                  style={{
                    fontSize: "var(--text-md)",
                    fontWeight: 600,
                    color: "var(--color-text-primary)",
                    whiteSpace: "nowrap",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                  }}
                >
                  {c.display_name || c.peer_display_name || "Unknown Peer"}
                </span>
                {c.last_message_at && (
                  <span
                    style={{
                      fontSize: "var(--text-xs)",
                      color: "var(--color-text-muted)",
                      flexShrink: 0,
                      marginLeft: 8,
                      fontWeight: 500,
                    }}
                  >
                    {formatRelativeTime(c.last_message_at)}
                  </span>
                )}
              </div>
              <span
                style={{
                  fontSize: "var(--text-sm)",
                  color: "var(--color-text-secondary)",
                  whiteSpace: "nowrap",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  marginTop: 2,
                }}
              >
                {c.last_message_preview || "No messages yet."}
              </span>
              {c.retention_policy !== "none" && (
                <span
                  style={{
                    fontSize: "var(--text-xs)",
                    color: "var(--color-warning)",
                    display: "flex",
                    alignItems: "center",
                    gap: 4,
                    marginTop: 2,
                  }}
                >
                  <span>⏳</span> Policy: {c.retention_policy}
                </span>
              )}
            </div>

            {/* Status dot */}
            <div
              title={c.is_online ? "Online" : "Offline"}
              style={{
                width: 8,
                height: 8,
                borderRadius: "50%",
                flexShrink: 0,
                background: c.is_online
                  ? "var(--color-success)"
                  : "var(--color-text-muted)",
                boxShadow: c.is_online
                  ? "0 0 6px var(--color-success)"
                  : "none",
                animation: c.is_online
                  ? "pulseDot 2s ease-in-out infinite"
                  : undefined,
              }}
            />

            {/* Actions */}
            <div
              className="conv-actions"
              style={{
                display: "flex",
                gap: "var(--space-xxs)",
                opacity: 0,
                transition: "opacity var(--transition-fast)",
              }}
              onMouseEnter={(e) => e.stopPropagation()}
            >
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  invoke("delete_conversation_cmd", {
                    conversationId: c.id,
                  })
                    .then(() => onDeleteConversation(c.id))
                    .catch(console.error);
                }}
                aria-label={`Delete conversation with ${c.display_name || c.peer_key_hex}`}
                style={{
                  padding: "4px 10px",
                  fontSize: "var(--text-xs)",
                  borderRadius: "var(--radius-sm)",
                  background: "transparent",
                  color: "var(--color-danger)",
                  border: "1px solid rgba(239,68,68,0.2)",
                  cursor: "pointer",
                  fontFamily: "inherit",
                  fontWeight: 500,
                  transition: "var(--transition-fast)",
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background =
                    "var(--color-danger-bg)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = "transparent";
                }}
              >
                Delete
              </button>
            </div>
          </div>
        ))
      )}
    </div>
  );
}

// ─── Helpers ───

function hashToColor(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = str.charCodeAt(i) + ((hash << 5) - hash);
  }
  const hue = Math.abs(hash) % 360;
  return `hsl(${hue}, 55%, 48%)`;
}

function formatRelativeTime(ts: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - ts;
  if (diff < 60) return "now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  return new Date(ts * 1000).toLocaleDateString();
}
