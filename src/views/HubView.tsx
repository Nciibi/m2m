import { ToastContainer } from "../toast";
import type {
  Toast,
  ConversationEntry,
  IdentityInfo,
  NetworkSettings,
} from "../types";

interface Props {
  // Shared state
  identity: IdentityInfo | null;
  toasts: Toast[];
  removeToast: (id: string) => void;

  // Invite state
  generatedInvite: string;
  inviteToConnect: string;
  inviteValid: boolean;
  namingMyName: string;
  namingTheirName: string;
  isConnecting: boolean;

  // Handlers
  onGenerateInvite: () => Promise<void>;
  onCopyInvite: () => void;
  setInviteToConnect: (v: string) => void;
  onConnect: () => Promise<void>;
  setNamingMyName: (v: string) => void;
  setNamingTheirName: (v: string) => void;
  onOpenChat: (conv: ConversationEntry) => void;
  onOpenSettings: () => void;
  onDeleteConversation: (id: string) => void;

  // Conversations
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
  return (
    <div className="app-container">
      <div className="header">
        <h1>
          <span>🛡️</span> M2M
        </h1>
        <div className="header-actions">
          <div className="status-badge">Offline</div>
          <button
            className="icon-btn"
            onClick={onOpenSettings}
            title="Settings"
            id="settings-btn"
          >
            ⚙️
          </button>
        </div>
      </div>
      <HubTabsContent
        {...{
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
          onDeleteConversation,
          conversations,
          networkSettings,
          privateMode,
          identity,
        }}
      />
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}

// ─── Sub-component: Tab Content ───

import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface TabProps {
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
  onDeleteConversation: (id: string) => void;
  conversations: ConversationEntry[];
  networkSettings: NetworkSettings | null;
  privateMode: boolean;
  identity: IdentityInfo | null;
}

function HubTabsContent(props: TabProps) {
  const [hubTab, setHubTab] = useState<"connect" | "chats">("connect");

  return (
    <>
      <div className="hub-tabs">
        <button
          className={`hub-tab ${hubTab === "connect" ? "active" : ""}`}
          onClick={() => setHubTab("connect")}
        >
          🔌 Connect
        </button>
        <button
          className={`hub-tab ${hubTab === "chats" ? "active" : ""}`}
          onClick={() => setHubTab("chats")}
        >
          💬 Chats
          {props.conversations.length > 0 && (
            <span className="tab-badge">{props.conversations.length}</span>
          )}
        </button>
      </div>

      <div className="content-area hub-tab-content">
        {hubTab === "connect" ? (
          <ConnectTab {...props} />
        ) : (
          <ChatsTab {...props} />
        )}
      </div>
    </>
  );
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
  setInviteToConnect,
  onConnect,
  setNamingMyName,
  setNamingTheirName,
  networkSettings,
  privateMode,
  identity,
}: TabProps) {
  return (
    <div className="centered-view">
      <div className="invite-section">
        {/* Host Card */}
        <div className="card" id="host-card">
          <div className="card-header">
            <div className="card-icon host">➕</div>
            <h3>Host a Connection</h3>
          </div>
          <p className="card-desc">
            Generate a one-time signed invite for a peer to connect to you
            securely.
          </p>
          {!generatedInvite ? (
            <button id="generate-invite-btn" onClick={onGenerateInvite}>
              Generate Invite Link
            </button>
          ) : (
            <div className="invite-output">
              <input readOnly value={generatedInvite} id="invite-output" />
              <button
                className="icon-btn"
                onClick={onCopyInvite}
                title="Copy to clipboard"
                id="copy-invite-btn"
              >
                📋
              </button>
            </div>
          )}
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
        </div>

        {/* Join Card */}
        <div className="card" id="join-card">
          <div className="card-header">
            <div className="card-icon join">🔗</div>
            <h3>Join a Connection</h3>
          </div>
          <p className="card-desc">
            Paste an invite link from a trusted peer to connect.
          </p>
          <div className="flex-row">
            <input
              id="invite-input"
              placeholder="m2m://..."
              value={inviteToConnect}
              onChange={(e) => setInviteToConnect(e.target.value)}
            />
            <button
              id="connect-btn"
              onClick={onConnect}
              disabled={isConnecting || !inviteToConnect}
            >
              {isConnecting ? "..." : "Connect"}
            </button>
          </div>
          {inviteValid && (
            <div className="naming-panel">
              <div className="valid-badge">✅ Valid Invite Found</div>
              <label>
                Your Display Name (optional)
                <input
                  placeholder="How they will see you"
                  value={namingMyName}
                  onChange={(e) => setNamingMyName(e.target.value)}
                />
              </label>
              <label>
                Their Display Name (optional)
                <input
                  placeholder="How you want to see them"
                  value={namingTheirName}
                  onChange={(e) => setNamingTheirName(e.target.value)}
                />
              </label>
            </div>
          )}
        </div>

        <div className="section-divider" />

        {/* Fingerprint */}
        <div className="fingerprint-box" id="fingerprint-display">
          <span className="fingerprint-label">Your Identity Fingerprint</span>
          {identity?.fingerprint}
        </div>
      </div>
    </div>
  );
}

function ChatsTab({
  conversations,
  onOpenChat,
  onDeleteConversation,
}: TabProps) {
  return (
    <div className="conversation-list">
      {conversations.length === 0 ? (
        <div className="conversation-list-empty">
          <span className="empty-icon">📭</span>
          No conversations yet. Connect to a peer to start chatting!
        </div>
      ) : (
        conversations.map((c) => (
          <div
            key={c.id}
            className="conversation-item"
            onClick={() => onOpenChat(c)}
          >
            <div className={`conv-avatar ${c.is_online ? "online" : ""}`}>
              {(c.display_name || c.peer_display_name || c.peer_key_hex).charAt(0)}
            </div>
            <div className="conv-body">
              <div className="conv-top-row">
                <span className="conv-name">
                  {c.display_name || c.peer_display_name || "Unknown Peer"}
                </span>
                {c.last_message_at && (
                  <span className="conv-time">
                    {new Date(c.last_message_at * 1000).toLocaleTimeString([], {
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </span>
                )}
              </div>
              <div className="conv-preview">
                {c.last_message_preview || "No messages yet."}
              </div>
              <div className="conv-retention-badge">
                {c.retention_policy !== "none" && `Policy: ${c.retention_policy}`}
              </div>
            </div>
            <div
              className={`conv-status-dot ${c.is_online ? "online" : "offline"}`}
            />
            <div className="conv-actions">
              <button
                className="danger"
                onClick={(e) => {
                  e.stopPropagation();
                  invoke("delete_conversation_cmd", { conversationId: c.id })
                    .then(() => onDeleteConversation(c.id))
                    .catch(console.error);
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
