import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, Input, Card, Badge, ToastContainer } from "../components/ui";
import {
  GearIcon, PlusIcon, LinkIcon, CopyIcon, CheckIcon,
  SearchIcon, MessageIcon, TrashIcon, OnlineDot, OfflineDot, HomeIcon, WifiIcon, ClockIcon,
  StarIcon, BellIcon, FolderIcon, AlertTriangleIcon,
} from "../components/ui/Icons";
import Sidebar from "../components/Sidebar";
import type { ToastData } from "../components/ui/Toast";
import { useApp } from "../context/AppContext";
import { useChat } from "../context/ChatContext";
import { asArray } from "../events";
import { useSettings } from "../context/SettingsContext";
import FamilyTab from "../components/FamilyTab";
import { useNow } from "../hooks/useNow";
import type {
  ConnectionInfo,
  ConversationEntry,
  DiscoveredPeer,
  DiscoveryConfig,
  FamilyMember,
  IdentityInfo,
  NetworkSettings,
  SecurityConfig,
} from "../types";
import { hashToColor, formatTime, copyToClipboard, errorMessage } from "../utils";

export default function HubView() {
  const { identity, setView, toasts, removeToast, addToast } = useApp();
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
  const [_familyLoading, setFamilyLoading] = useState(false);

const handleCopy = async (): Promise<boolean> => {
    // `setCopied(true)` used to be unconditional, so the green ✓ the user's
    // only confirmation that a 60-minute one-time invite actually reached the
    // clipboard — appeared even when the write was refused.
    const ok = await copyInvite();
    if (!ok) return false;
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
    if (securityConfig?.clipboard_clear_secs && securityConfig.clipboard_clear_secs > 0) {
      scheduleClipboardClear(securityConfig.clipboard_clear_secs);
    }
    return true;
  };

  const loadFamily = useCallback(async () => {
    try {
      setFamilyLoading(true);
      // `asArray`: asserts vs. checks — see `events.ts`.
      const f = asArray<FamilyMember>(await invoke("list_family"));
      setFamily(f);
    } catch { /* noop */ }
    finally { setFamilyLoading(false); }
  }, []);

  const handleFamilyConnect = useCallback(async (peerKeyHex: string) => {
    // connect emits m2m://connection event which ChatContext picks up
    await invoke("connect_family_member", { peerKeyHex });
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

  // Global keyboard shortcuts for Hub
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;
      if (ctrl && e.key === "n") {
        e.preventDefault();
        setTab("connect");
      }
      if (e.key === "Escape") {
        // Already on hub — no-op
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
    <div className="app-shell">
      <Sidebar currentView="hub" onNavigate={setView} onError={(m) => addToast(m, "error", 6000)} />
      <div className="app-main">
      <div className="app-header">
        <h1 className="app-header__title">
          <span className="app-header__icon-bg app-header__icon-bg--accent">
            <img src="logo.svg" alt="M2M" width="20" height="20" style={{ borderRadius: '4px' }} />
          </span>
          M2M
        </h1>
        <div className="app-header__actions">
          <Badge variant={connectionBadge.variant} compact>
            {connectionBadge.dot} {connectionBadge.label}
          </Badge>
          <button className="btn btn--icon" onClick={openSettings} id="settings-btn" aria-label="Settings"><GearIcon size={20} /></button>
        </div>
      </div>

      {/*
        Tabs: `aria-controls` + roving `tabIndex` + arrow-key navigation, per
        the WAI-ARIA tabs pattern. Previously each tab was a separate tab stop,
        there was no `aria-controls`, no `tabpanel` on the content region, and
        no arrow-key support — so a keyboard or screen-reader user had to Tab
        four times to reach a tab and had no indication of what the tab
        controlled.

        `onKeyDown` implements Left/Right/Home/End, and focuses the newly
        selected tab via the data attribute.
      */}
      <div className="tab-bar" role="tablist" aria-label="Sections" onKeyDown={(e) => {
        const order = ["connect", "chats", "nearby", "family"] as const;
        const i = order.indexOf(tab);
        let next: number | null = null;
        if (e.key === "ArrowRight") next = (i + 1) % order.length;
        else if (e.key === "ArrowLeft") next = (i - 1 + order.length) % order.length;
        else if (e.key === "Home") next = 0;
        else if (e.key === "End") next = order.length - 1;
        if (next === null) return;
        e.preventDefault();
        setTab(order[next]);
        document.querySelector<HTMLElement>(`[data-tab="${order[next]}"]`)?.focus();
      }}>
        <button className={`tab-bar__tab ${tab === "connect" ? "tab-bar__tab--active" : ""}`} onClick={() => setTab("connect")} role="tab" id={`tab-connect`} aria-controls="tabpanel-main" aria-selected={tab === "connect"} tabIndex={tab === "connect" ? 0 : -1} data-tab={"connect"}>
          <LinkIcon size={16} /> Connect
        </button>
        <button className={`tab-bar__tab ${tab === "chats" ? "tab-bar__tab--active" : ""}`} onClick={() => setTab("chats")} role="tab" id={`tab-chats`} aria-controls="tabpanel-main" aria-selected={tab === "chats"} tabIndex={tab === "chats" ? 0 : -1} data-tab={"chats"}>
          <MessageIcon size={16} /> Chats
          {conversations.length > 0 && <span className="tab-bar__badge">{conversations.length}</span>}
        </button>
        <button className={`tab-bar__tab ${tab === "nearby" ? "tab-bar__tab--active" : ""}`} onClick={() => setTab("nearby")} role="tab" id={`tab-nearby`} aria-controls="tabpanel-main" aria-selected={tab === "nearby"} tabIndex={tab === "nearby" ? 0 : -1} data-tab={"nearby"}>
          <WifiIcon size={16} /> Nearby
          {discoveredPeers.length > 0 && <span className="tab-bar__badge">{discoveredPeers.length}</span>}
        </button>
        <button className={`tab-bar__tab ${tab === "family" ? "tab-bar__tab--active" : ""}`} onClick={() => setTab("family")} role="tab" id={`tab-family`} aria-controls="tabpanel-main" aria-selected={tab === "family"} tabIndex={tab === "family" ? 0 : -1} data-tab={"family"}>
          <HomeIcon size={16} /> Family
          {family.length > 0 && <span className="tab-bar__badge">{family.length}</span>}
        </button>
      </div>

      <div className="app-content" id="tabpanel-main" role="tabpanel" aria-labelledby={`tab-${tab}`} tabIndex={0}>
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
          addToast={addToast}
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
          <ChatsTab conversations={filtered} onOpenChat={handleOpenChat} onDeleteConversation={handleDeleteConversation} search={search} setSearch={setSearch} onGetStarted={() => setTab("connect")} mutedConversations={mutedConversations} onMute={handleMuteConversation} onUnmute={handleUnmuteConversation} addToast={addToast} />
        )}
      </div>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
      </div>
    </div>
  );
}

/** Props for the Connect tab.
 *
 *  This was `any` with 18 destructured properties — which is not a style
 *  complaint: with `any` props, TypeScript could not catch
 *  `onDeleteConversation(c.id)` being called against a handler declared as
 *  `() => void`, and the `conversationId` was silently discarded.
 *
 *  Adding the annotation was not sufficient on its own. A zero-parameter
 *  function is assignable to `(id: string) => void`, so the compiler stayed
 *  quiet while the id continued to be dropped: the delete only happened because
 *  `invoke("delete_conversation_cmd")` was inlined at the call site, and
 *  `handleDeleteConversation` merely reloaded the list. The handler now accepts
 *  and uses the id itself.
 */
interface ConnectTabProps {
  generatedInvite: string;
  inviteToConnect: string;
  inviteValid: boolean;
  namingMyName: string;
  namingTheirName: string;
  isConnecting: boolean;
  /** Resolves to the new invite string, or `null` if none was created. Never a
   *  bare `void` and never just `boolean`: the caller needs to record the
   *  invite in history, and it needs to do so *after* the await, when the
   *  `generatedInvite` prop has not re-rendered yet. A void return made a
   *  failed generation indistinguishable from a successful one. */
  onGenerateInvite: () => Promise<string | null>;
  /** Resolves true only when the invite text actually reached the clipboard. */
  onCopyInvite: () => Promise<boolean>;
  copied: boolean;
  setInviteToConnect: (v: string) => void;
  onConnect: () => void;
  setNamingMyName: (v: string) => void;
  setNamingTheirName: (v: string) => void;
  networkSettings: NetworkSettings | null;
  privateMode: boolean;
  identity: IdentityInfo | null;
  securityConfig: SecurityConfig | null;
  scheduleClipboardClear: (secs: number) => void;
  /** Report a failed action. Copy and favourite failures must never be silent. */
  addToast: (msg: string, type?: ToastData["type"], duration?: number) => void;
}

function ConnectTab({
  generatedInvite, inviteToConnect, inviteValid, namingMyName, namingTheirName,
  isConnecting, onGenerateInvite, onCopyInvite, copied, setInviteToConnect, onConnect,
  setNamingMyName, setNamingTheirName, networkSettings, privateMode, identity,
  securityConfig, scheduleClipboardClear, addToast,
}: ConnectTabProps) {
  const [generating, setGenerating] = useState(false);
  const [fpCopied, setFpCopied] = useState(false);
  const [inviteHistory, setInviteHistory] = useState<string[]>([]);
  const [inviteCreatedAt, setInviteCreatedAt] = useState<number | null>(null);
  const [inviteExpiry, setInviteExpiry] = useState<number>(60);
  const [isListening, setIsListening] = useState(false);

  // Check if we're listening
  useEffect(() => {
    invoke<string>("get_listen_address").then((addr) => {
      setIsListening(!!addr && addr !== "Not listening");
    }).catch(() => {});
  }, []);

  // Invite countdown, derived rather than stored.
  //
  // This was `useState` + a `setInterval` that pushed a new value every second,
  // which cost a render per second and left the label showing the previous
  // second's value on every render. `useNow` supplies the clock, so the
  // remaining time is computed during render and is correct by construction.
  const inviteNow = useNow(1000);
  const expiryRemaining = inviteCreatedAt
    ? Math.max(0, inviteExpiry * 60 - (inviteNow / 1000 - inviteCreatedAt))
    : 0;

  const handleGenerate = async () => {
    setGenerating(true);
    try {
      // The await is load-bearing. It used to await `undefined`, so `finally`
      // cleared `generating` before the invite existed and the three state
      // writes below ran unconditionally — on the failure path too.
      const invite = await onGenerateInvite();
      // Check the value, not just `null`: a `void`-returning caller (a stale
      // mock, or plain JS) would otherwise push `undefined` into the history
      // and every `inv.substring` in the list would throw.
      if (typeof invite !== "string" || invite.length === 0) return;
      setInviteCreatedAt(Date.now() / 1000);
      setInviteExpiry(60);
      setIsListening(true);
      pushInviteHistory(invite);
    } finally { setGenerating(false); }
  };

  // Recent-invite history. Appended at the point of a *successful* generation
  // rather than accumulated by an effect on `generatedInvite`, which was a
  // second copy of the same derivation running a render late — and which fired
  // for invites that had in fact failed, because it could not tell.
  const pushInviteHistory = (invite: string) => {
    setInviteHistory((prev) => [invite, ...prev.filter((i) => i !== invite)].slice(0, 5));
  };

  return (
    <div className="centered-view">
      <div className="invite-section">
        <Card header={{ icon: <PlusIcon size={18} color="var(--color-accent-bright)" />, title: "Host a Connection" }} description="Generate a one-time signed invite for a peer to connect to you securely.">
          {isListening && (
            <div className="listening-indicator">
              <span className="listening-indicator__dot" />
              Listening for incoming connections
            </div>
          )}
          {!generatedInvite ? (
            <Button id="generate-invite-btn" onClick={handleGenerate} loading={generating}>Generate Invite Link</Button>
          ) : (
            <>
              <div className="invite-output">
                <div className="invite-output__field">
                  <span className="invite-output__text">{generatedInvite}</span>
                </div>
                <button className={`btn btn--icon ${copied ? 'btn--icon-copied' : ''}`} onClick={onCopyInvite} id="copy-invite-btn" aria-label="Copy invite">
                  {copied ? <span className="copied-pop"><CheckIcon size={18} /></span> : <CopyIcon size={18} />}
                </button>
              </div>
              {expiryRemaining > 0 && (
                <div className="invite-countdown">
                  <ClockIcon size={14} />
                  Expires in {Math.floor(expiryRemaining / 60)}m:{Math.floor(expiryRemaining % 60).toString().padStart(2, "0")}
                </div>
              )}
            </>
          )}
          {inviteHistory.length > 0 && (
            <div className="invite-history">
              <div className="invite-history__title">Recent Invites</div>
              {inviteHistory.map((inv, i) => (
                <div key={i} className="invite-history__item" role="button" tabIndex={0}
                  onClick={() => {
                    void copyToClipboard(inv).then((ok) => {
                      if (!ok) { addToast("Could not copy to the clipboard", "error"); return; }
                      if (securityConfig?.clipboard_clear_secs && securityConfig.clipboard_clear_secs > 0) {
                        scheduleClipboardClear(securityConfig.clipboard_clear_secs);
                      }
                    });
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      e.currentTarget.click();
                    }
                  }}>
                  <span>{inv.substring(0, 40)}…</span>
                  <CopyIcon size={12} />
                </div>
              ))}
            </div>
          )}
          {networkSettings?.tor_enabled && !privateMode && generatedInvite && (
            <div className="tor-warning">
              <AlertTriangleIcon size={18} />
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
              if (!identity?.fingerprint) return;
              void copyToClipboard(identity.fingerprint).then((ok) => {
                if (!ok) { addToast("Could not copy to the clipboard", "error"); return; }
                setFpCopied(true);
                setTimeout(() => setFpCopied(false), 2000);
              });
            }} aria-label="Copy">
              {fpCopied ? <span className="copied-pop"><CheckIcon size={14} /></span> : <CopyIcon size={14} />}
            </button>
          </span>
        </div>
      </div>
    </div>
  );
}

interface ChatsTabProps {
  conversations: ConversationEntry[];
  onOpenChat: (c: ConversationEntry) => void;
  onDeleteConversation: (conversationId: string) => void;
  search: string;
  setSearch: (v: string) => void;
  onGetStarted: () => void;
  mutedConversations: string[];
  onMute: (peerKeyHex: string) => void;
  onUnmute: (peerKeyHex: string) => void;
  /** Report a failed action. Favourite/archive writes must never be silent. */
  addToast: (msg: string, type?: ToastData["type"], duration?: number) => void;
}

function ChatsTab({
  conversations, onOpenChat, onDeleteConversation, search, setSearch, onGetStarted,
  mutedConversations, onMute, onUnmute, addToast,
}: ChatsTabProps) {
  // Favourite/archive are derived from the conversation list, with local
  // *overrides* layered on top for the window between a successful toggle and
  // the next `loadConversations`.
  //
  // These were `useState<Set>` mirrored by an effect keyed on `conversations`,
  // which was a real bug and not just an extra render: any conversation refresh
  // landing between the toggle's `invoke` resolving and the server state
  // catching up replaced the whole Set, so the star the user had just clicked
  // silently reverted. An override cannot be reverted by a refresh, because a
  // refresh no longer writes to it.
  const [favOverride, setFavOverride] = useState<Record<string, boolean>>({});
  const [archOverride, setArchOverride] = useState<Record<string, boolean>>({});

  const favorites = useMemo(
    () => new Set(conversations.filter((c) => favOverride[c.peer_key_hex] ?? c.is_favorite).map((c) => c.peer_key_hex)),
    [conversations, favOverride],
  );
  const archived = useMemo(
    () => new Set(conversations.filter((c) => archOverride[c.peer_key_hex] ?? c.archived).map((c) => c.peer_key_hex)),
    [conversations, archOverride],
  );

  const toggleFav = async (peerKeyHex: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      const newVal = await invoke<boolean>("toggle_favorite", { peerKeyHex });
      setFavOverride((prev) => ({ ...prev, [peerKeyHex]: newVal }));
    } catch (err) {
      // Not cosmetic. `catch {}` here meant a failed write left the star
      // un-moved and said nothing, so the user's only evidence was their own
      // click — and CLAUDE.md names favourites explicitly as a control that
      // must not silently fail. Note the optimistic update above is applied
      // only after the invoke resolves, so nothing needs rolling back.
      addToast("Could not update favourite: " + errorMessage(err), "error");
    }
  };

  const toggleArch = async (peerKeyHex: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      const newVal = await invoke<boolean>("toggle_archive", { peerKeyHex });
      setArchOverride((prev) => ({ ...prev, [peerKeyHex]: newVal }));
    } catch (err) {
      addToast("Could not update archive: " + errorMessage(err), "error");
    }
  };
  // Sort conversations: favorites first, then by recency, archived at bottom
  const sorted = [...conversations].sort((a: ConversationEntry, b: ConversationEntry) => {
    if ((a.archived ? 1 : 0) !== (b.archived ? 1 : 0)) return (a.archived ? 1 : 0) - (b.archived ? 1 : 0);
    if ((a.is_favorite ? 1 : 0) !== (b.is_favorite ? 1 : 0)) return (b.is_favorite ? 1 : 0) - (a.is_favorite ? 1 : 0);
    return (b.last_message_at || 0) - (a.last_message_at || 0);
  });

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
          <p className="conv-empty__title">{search ? "No conversations found" : "No conversations yet"}</p>
          <p className="conv-empty__desc">
            {search
              ? "Try adjusting your search terms or clear the filter."
              : "Generate an invite link to host a connection, or paste an invite from a peer to join."}
          </p>
          {!search && (
            <Button onClick={onGetStarted} icon={<PlusIcon size={18} />}>
              Get Started
            </Button>
          )}
        </div>
      ) : (
        sorted.map((c) => {
          const isMuted = mutedConversations?.includes(c.peer_key_hex);
          return (
          <div key={c.id} className="conv-item" onClick={() => onOpenChat(c)} role="button" tabIndex={0} onKeyDown={e => e.key === "Enter" && onOpenChat(c)}>
            <div className="conv-avatar-wrap">
              <div className={`conv-avatar ${c.is_online ? 'conv-avatar--online' : ''}`} style={{
                background: `linear-gradient(135deg, ${hashToColor(c.peer_key_hex)}, ${hashToColor(c.peer_key_hex.slice(16))})`,
              }}>
                {(c.display_name || c.peer_display_name || c.peer_key_hex).charAt(0).toUpperCase()}
              </div>
              {c.is_online && <span className="online-dot" />}
            </div>
            <div className="conv-body">
              <div className="conv-top">
                <span className="conv-name">{c.display_name || c.peer_display_name || "Unknown Peer"}{isMuted ? <BellIcon size={12} off className="mute-indicator" /> : null}</span>
                {c.last_message_at && <span className="relative-time">{formatTime(c.last_message_at)}</span>}
              </div>
              <span className="conv-preview">{c.last_message_preview || "No messages yet."}</span>
            </div>
            <div className="conv-actions">
              {/* Favorite toggle */}
              <button className="btn btn--icon btn--icon-sm" title={favorites.has(c.peer_key_hex) ? "Unfavorite" : "Favorite"}
                onClick={(e) => toggleFav(c.peer_key_hex, e)}
                aria-label={favorites.has(c.peer_key_hex) ? "Unfavorite" : "Favorite"}>
                <StarIcon size={16} filled={favorites.has(c.peer_key_hex)} />
              </button>
              {/* Archive toggle */}
              <button className="btn btn--icon btn--icon-sm" title={archived.has(c.peer_key_hex) ? "Unarchive" : "Archive"}
                onClick={(e) => toggleArch(c.peer_key_hex, e)}
                aria-label={archived.has(c.peer_key_hex) ? "Unarchive" : "Archive"}>
                <FolderIcon size={16} open={archived.has(c.peer_key_hex)} />
              </button>
              <button className="btn btn--icon btn--icon-sm"
                title={isMuted ? "Unmute conversation" : "Mute conversation"}
                onClick={e => { e.stopPropagation(); if (isMuted) { onUnmute(c.peer_key_hex); } else { onMute(c.peer_key_hex); } }}
                aria-label={isMuted ? "Unmute" : "Mute"}>
                <BellIcon size={16} off={isMuted} />
              </button>
              <button className="btn btn--icon btn--icon-sm"
                onClick={e => { e.stopPropagation(); void onDeleteConversation(c.id); }}
                aria-label="Delete">
                <TrashIcon size={16} />
              </button>
            </div>
          </div>
          );
        })
      )}
    </div>
  );
}

interface NearbyTabProps {
  discoveryConfig: DiscoveryConfig | null;
  discoveredPeers: DiscoveredPeer[];
  /** Resolves with the established connection, or null if none was made. */
  onConnect: (address: string) => Promise<ConnectionInfo | null>;
  onRefresh: () => void;
  onOpenSettings: () => void;
  onOpenChat: (c: ConversationEntry) => void;
}

function NearbyTab({
  discoveryConfig, discoveredPeers, onConnect, onRefresh, onOpenSettings, onOpenChat,
}: NearbyTabProps) {
  const [connecting, setConnecting] = useState<string | null>(null);

  const handleConnectPeer = async (address: string) => {
    setConnecting(address);
    try {
      const info = await onConnect(address);
      if (info?.peer_key_hex) {
        onOpenChat({
          id: info.peer_key_hex,
          peer_key_hex: info.peer_key_hex,
          display_name: null,
          peer_display_name: null,
          last_message_at: null,
          last_message_preview: null,
          message_count: 0,
          is_online: true,
          auto_delete_at: null,
          retention_policy: "none",
          created_at: 0,
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
        <div className="conv-empty">
          <p className="conv-empty__title">Discovery Not Active</p>
          <p className="conv-empty__desc">
            Enable LAN or DHT discovery in Settings to find nearby peers.
            Both are <strong>OFF by default</strong> — privacy first.
          </p>
          <Button variant="secondary" size="sm" onClick={onOpenSettings}>
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
        <div className="conv-empty">
          <WifiIcon size={48} color="var(--color-text-muted)" />
          <p className="conv-empty__title">No Peers Found</p>
          <p className="conv-empty__desc">
            {discoveryConfig?.lan_enabled
              ? "No LAN peers detected. Make sure other M2M users are on the same network with LAN discovery enabled."
              : ""}
            {discoveryConfig?.lan_enabled && discoveryConfig?.dht_enabled ? " " : ""}
            {discoveryConfig?.dht_enabled
              ? "No DHT peers found. They may be offline or behind a symmetric NAT."
              : ""}
          </p>
          <Button variant="secondary" size="xs" onClick={onRefresh}>Refresh</Button>
        </div>
      </div>
    );
  }

  return (
    <div className="conv-list">
      <div className="nearby-actions">
        <Button variant="secondary" size="xs" onClick={onRefresh}>Refresh</Button>
      </div>
      {discoveredPeers.map((peer, idx) => (
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
              <span className={`badge badge--${peer.method === "lan" ? "info" : "warning"} badge--inline`}>
                {peer.method === "lan" ? "LAN" : "DHT"}
              </span>
            </div>
            <div className="conv-preview conv-preview--mono">
              {peer.id_hex.slice(0, 16)}...
            </div>
          </div>
          <div className="conv-status conv-status--actions">
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

