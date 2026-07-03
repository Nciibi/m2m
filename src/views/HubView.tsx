import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ToastContainer, NoChatsIllustration, RadarIllustration } from "../components/ui";
import { useApp } from "../context/AppContext";
import { useChat } from "../context/ChatContext";
import FamilyTab from "../components/FamilyTab";
import type { FamilyMember, ConnectionInfo } from "../types";
import { formatTime } from "../utils";

export default function HubView() {
  const { identity, toasts, removeToast, addToast, setView } = useApp();
  const { conversations, handleOpenChat: ctxHandleOpenChat } = useChat();
  const [activeTab, setActiveTab] = useState<"connect" | "chats" | "nearby" | "family">("connect");
  const [familyMembers, setFamilyMembers] = useState<FamilyMember[]>([]);

  const refreshFamily = async () => {
    try { setFamilyMembers(await invoke<FamilyMember[]>("list_family")); } catch {}
  };
  const connectFamily = async (peerKeyHex: string) => {
    try { await invoke("connect_family_member", { peerKeyHex }); setView("chat"); } catch (e) { throw e; }
  };

  useEffect(() => { refreshFamily(); }, []);

  // State for Connect Tab
  const [generatedInvite, setGeneratedInvite] = useState("");
  const [inviteToConnect, setInviteToConnect] = useState("");
  const [namingMyName, setNamingMyName] = useState("");
  const [namingTheirName, setNamingTheirName] = useState("");
  const [isConnecting, setIsConnecting] = useState(false);
  const [isGenerating, setIsGenerating] = useState(false);
  const [copied, setCopied] = useState(false);
  const inviteValid = inviteToConnect.startsWith("m2m://") && inviteToConnect.length > 20;

  // Search for Chats
  const [search, setSearch] = useState("");

  const handleGenerateInvite = async () => {
    setIsGenerating(true);
    try {
      // Ensure listener is running to get an address
      await invoke("start_listening", { address: "0.0.0.0:0" });
      const address = await invoke<string>("get_listen_address");
      const invite = await invoke<string>("create_invite", { address, validityMinutes: 60, oneTime: false });
      setGeneratedInvite(invite);
      addToast("Invite link generated!", "success");
    } catch (e: any) {
      const msg = typeof e === "string" ? e : e?.message || "Failed to generate invite link";
      addToast(msg, "error");
    } finally {
      setIsGenerating(false);
    }
  };

  const handleCopyInvite = () => {
    if (generatedInvite) {
      navigator.clipboard.writeText(generatedInvite);
      setCopied(true);
      addToast("Copied to clipboard!", "success");
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const handleConnect = async () => {
    if (!inviteValid) return;
    setIsConnecting(true);
    try {
      const info = await invoke<ConnectionInfo>("connect_to_peer", { inviteStr: inviteToConnect });
      // Set display names after connecting
      if (info.peer_key_hex && (namingTheirName || namingMyName)) {
        try {
          if (namingMyName || namingTheirName) {
            await invoke("send_conversation_names", {
              peerKeyHex: info.peer_key_hex,
              myName: namingMyName || "Me",
              theirName: namingTheirName || "Them",
            });
          }
        } catch (e) {
          console.warn("Failed to set display names:", e);
        }
      }
      setView("chat");
    } catch (e: any) {
      const msg = typeof e === "string" ? e : e?.message || "Connection failed";
      addToast(msg, "error");
    } finally {
      setIsConnecting(false);
    }
  };

  const handleOpenChat = (peerKeyHex: string) => {
    const conv = conversations.find(c => c.peer_key_hex === peerKeyHex);
    if (conv) {
      ctxHandleOpenChat(conv);
    }
  };

  const handleDeleteConversation = async (e: React.MouseEvent, peerKeyHex: string) => {
    e.stopPropagation();
    try {
      await invoke("delete_conversation_cmd", { conversationId: peerKeyHex });
      addToast("Conversation deleted", "success");
    } catch (err: any) {
      addToast("Failed to delete conversation", "error");
    }
  };

  return (
    <main className="premium-glass-card w-full h-full flex flex-col relative z-10">
      {/* Header */}
      <header className="h-[56px] px-xl flex items-center justify-between border-b border-border-subtle shrink-0 bg-surface/40 backdrop-blur-3xl">
        <div className="flex items-center gap-md">
          <div className="w-8 h-8 rounded-lg bg-gradient-to-tr from-primary-container to-inverse-primary flex items-center justify-center shadow-[0_0_15px_rgba(99,102,241,0.4)]">
            <span className="material-symbols-outlined text-white text-[18px]" style={{ fontVariationSettings: "'FILL' 1" }}>security</span>
          </div>
          <span className="font-headline-2xl text-headline-2xl font-extrabold tracking-tight animate-text-shimmer">M2M</span>
        </div>
        <div className="flex items-center gap-lg">
          <div className="flex items-center gap-sm bg-surface-container-low/50 px-md py-xs rounded-full border border-border-subtle">
            <div className="w-2 h-2 rounded-full bg-tertiary animate-pulse"></div>
            <span className="font-label-sm text-label-sm text-on-surface-variant">Online</span>
          </div>
          <button onClick={() => setView("settings")} className="text-on-surface-variant hover:text-primary transition-colors active:scale-95 p-1 rounded-lg hover:bg-input-bg">
            <span className="material-symbols-outlined text-[20px]">settings</span>
          </button>
        </div>
      </header>

      {/* Tab Bar */}
      <nav className="h-[44px] px-xl flex items-center border-b border-border-subtle shrink-0">
        <div className="flex items-center h-full gap-xl">
          {([
            { id: "connect" as const, icon: "link", label: "Connect" },
            { id: "chats" as const, icon: "chat_bubble", label: "Chats", badge: conversations.reduce((sum, c) => sum + (c.unread_count || 0), 0) },
            { id: "nearby" as const, icon: "wifi", label: "Nearby" },
            { id: "family" as const, icon: "group", label: "Family" },
          ]).map(tab => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`h-full flex items-center gap-sm px-xs border-b-2 transition-all duration-200 ${
                activeTab === tab.id
                  ? "border-primary text-on-surface"
                  : "border-transparent text-on-surface-variant hover:text-on-surface"
              }`}
            >
              <span className="material-symbols-outlined text-[18px]">{tab.icon}</span>
              <span className={`font-label-sm text-label-sm ${activeTab === tab.id ? "font-bold" : ""}`}>{tab.label}</span>
              {tab.badge && tab.badge > 0 ? (
                <span className="bg-primary-container text-on-primary-container text-[10px] px-1.5 py-0.5 rounded-full font-bold min-w-[18px] text-center">{tab.badge}</span>
              ) : null}
            </button>
          ))}
        </div>
      </nav>

      {/* Main Content Area — fills remaining space and scrolls */}
      <div className="flex-1 overflow-y-auto custom-scrollbar">
        <div className="p-xl">

          {/* ─── CONNECT TAB ─── */}
          {activeTab === "connect" && (
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-xl">
              {/* Host Connection */}
              <section className="glass-card rounded-2xl p-xl flex flex-col">
                <div className="flex items-center gap-md mb-xl">
                  <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center border border-primary/20">
                    <span className="material-symbols-outlined text-primary">broadcast_on_personal</span>
                  </div>
                  <div>
                    <h2 className="font-headline-2xl text-headline-2xl text-on-surface">Host a Connection</h2>
                    <p className="font-body-md text-body-md text-on-surface-variant">Create a secure link for others to join.</p>
                  </div>
                </div>

                <button
                  onClick={handleGenerateInvite}
                  disabled={isGenerating}
                  className="premium-btn w-full py-md px-xl bg-gradient-to-r from-primary-container to-inverse-primary text-white rounded-xl font-headline-2xl text-headline-2xl font-bold flex items-center justify-center gap-md hover:brightness-125 transition-all duration-300 shadow-[0_0_20px_rgba(99,102,241,0.2)] hover:shadow-[0_0_30px_rgba(99,102,241,0.5)] mb-lg border border-outline-variant disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  <span className="material-symbols-outlined relative z-10">{isGenerating ? "sync" : generatedInvite ? "refresh" : "add_link"}</span>
                  <span className="relative z-10">{isGenerating ? "Generating..." : generatedInvite ? "Regenerate Invite" : "Generate Invite Link"}</span>
                </button>

                {generatedInvite && (
                  <div className="space-y-md animate-in fade-in slide-in-from-top-2 duration-300">
                    <div className="flex justify-between items-center">
                      <label className="font-label-sm text-label-sm text-on-surface-variant uppercase tracking-wider">Active Token</label>
                      <span className="flex items-center gap-xs font-mono-label text-[10px] text-tertiary">
                        <span className="w-1.5 h-1.5 rounded-full bg-tertiary animate-pulse"></span>
                        Awaiting Peer
                      </span>
                    </div>
                    <div className="flex flex-col gap-sm bg-input-bg p-md rounded-xl border border-outline-variant relative overflow-hidden group/token">
                      <div className="absolute top-0 right-0 p-xs font-mono-label text-[9px] text-text-muted/40 uppercase tracking-widest pointer-events-none">ECDH exchange</div>
                      <span className="font-mono-code text-mono-code text-primary break-all select-all pt-xs leading-relaxed">{generatedInvite}</span>
                      <div className="flex justify-end border-t border-border-subtle pt-sm mt-xs">
                        <button onClick={handleCopyInvite} className="flex items-center gap-xs text-on-surface-variant hover:text-primary transition-colors py-xs px-md rounded-lg hover:bg-bg-hover active:scale-95 transition-transform text-sm font-semibold">
                          <span className="material-symbols-outlined text-[18px]">{copied ? "check" : "content_copy"}</span>
                          <span>{copied ? "Copied!" : "Copy Token"}</span>
                        </button>
                      </div>
                    </div>
                  </div>
                )}

                {/* Atmospheric ping rings */}
                {!generatedInvite && (
                  <div className="flex-1 flex items-center justify-center pt-xl">
                    <div className="relative w-24 h-24 flex items-center justify-center opacity-20">
                      <div className="absolute inset-0 border border-primary/30 rounded-full animate-ping"></div>
                      <div className="absolute inset-4 border border-primary/20 rounded-full animate-[ping_2s_infinite]"></div>
                      <span className="material-symbols-outlined text-primary text-3xl">vpn_lock</span>
                    </div>
                  </div>
                )}
              </section>

              {/* Join Connection */}
              <section className="glass-card rounded-2xl p-xl flex flex-col">
                <div className="flex items-center gap-md mb-xl">
                  <div className="w-10 h-10 rounded-xl bg-tertiary/10 flex items-center justify-center border border-tertiary/20">
                    <span className="material-symbols-outlined text-tertiary">key</span>
                  </div>
                  <div>
                    <h2 className="font-headline-2xl text-headline-2xl text-on-surface">Join a Connection</h2>
                    <p className="font-body-md text-body-md text-on-surface-variant">Enter a link to start an encrypted chat.</p>
                  </div>
                </div>

                <div className="space-y-lg flex-1">
                  <div className="space-y-sm">
                    <label className="font-label-sm text-label-sm text-on-surface-variant uppercase tracking-wider">Invite Link</label>
                    <input
                      value={inviteToConnect}
                      onChange={e => setInviteToConnect(e.target.value)}
                      className="w-full bg-input-bg border border-outline-variant rounded-xl px-lg py-md text-primary font-mono-code focus:ring-2 focus:ring-primary placeholder:text-outline-variant outline-none transition-all"
                      placeholder="m2m://..."
                      type="text"
                    />
                    {inviteValid && (
                      <div className="flex items-center gap-sm text-tertiary font-label-sm text-label-sm px-xs animate-in fade-in duration-200">
                        <span className="material-symbols-outlined text-[16px]">check_circle</span>
                        <span>Valid Invite Found</span>
                      </div>
                    )}
                  </div>

                  <div className="grid grid-cols-2 gap-lg">
                    <div className="space-y-sm">
                      <label className="font-label-sm text-label-sm text-on-surface-variant uppercase tracking-wider">Your Name</label>
                      <input
                        value={namingMyName}
                        onChange={e => setNamingMyName(e.target.value)}
                        className="w-full bg-input-bg border border-outline-variant rounded-xl px-lg py-md text-on-surface focus:ring-2 focus:ring-primary outline-none transition-all"
                        placeholder="Optional"
                        type="text"
                      />
                    </div>
                    <div className="space-y-sm">
                      <label className="font-label-sm text-label-sm text-on-surface-variant uppercase tracking-wider">Their Name</label>
                      <input
                        value={namingTheirName}
                        onChange={e => setNamingTheirName(e.target.value)}
                        className="w-full bg-input-bg border border-outline-variant rounded-xl px-lg py-md text-on-surface focus:ring-2 focus:ring-primary outline-none transition-all"
                        placeholder="Optional"
                        type="text"
                      />
                    </div>
                  </div>

                  <button
                    onClick={handleConnect}
                    disabled={!inviteValid || isConnecting}
                    className="premium-btn w-full py-md px-xl bg-gradient-to-r from-tertiary-container to-tertiary text-white rounded-xl font-headline-2xl text-headline-2xl font-bold flex items-center justify-center gap-md hover:brightness-125 transition-all duration-300 shadow-[0_0_20px_rgba(16,185,129,0.15)] hover:shadow-[0_0_30px_rgba(16,185,129,0.4)] border border-outline-variant disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    <span className={`material-symbols-outlined relative z-10 ${isConnecting ? 'animate-spin' : ''}`}>
                      {isConnecting ? "sync" : "sensors"}
                    </span>
                    <span className="relative z-10">{isConnecting ? "Connecting..." : "Connect"}</span>
                  </button>
                </div>
              </section>
            </div>
          )}

          {/* ─── CHATS TAB ─── */}
          {activeTab === "chats" && (
            <div className="flex flex-col gap-lg">
              {/* Search */}
              <div className="relative">
                <span className="material-symbols-outlined absolute left-md top-1/2 -translate-y-1/2 text-on-surface-variant text-[20px]">search</span>
                <input
                  value={search}
                  onChange={e => setSearch(e.target.value)}
                  className="w-full h-[40px] bg-input-bg border border-outline-variant rounded-xl pl-10 pr-md text-body-md text-on-surface focus:ring-2 focus:ring-primary outline-none transition-all"
                  placeholder="Search conversations…"
                  type="text"
                />
              </div>

              {/* Conversation List */}
              <div className="space-y-sm">
                {conversations
                  .filter(c => c.peer_key_hex.includes(search) || (c.display_name && c.display_name.toLowerCase().includes(search.toLowerCase())))
                  .map((c) => (
                    <div
                      key={c.peer_key_hex}
                      onClick={() => handleOpenChat(c.peer_key_hex)}
                      className="group inner-glass p-md rounded-xl flex items-center gap-md cursor-pointer hover:bg-bg-hover transition-all duration-200"
                    >
                      <div className="relative flex-shrink-0">
                        <div className="w-12 h-12 rounded-full bg-gradient-to-br from-primary to-[#a855f7] flex items-center justify-center font-bold text-white text-lg">
                          {(c.display_name || c.peer_key_hex).charAt(0).toUpperCase()}
                        </div>
                        {c.is_online && <div className="absolute top-0 right-0 w-3 h-3 bg-tertiary border-2 border-surface-container-lowest rounded-full"></div>}
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="flex justify-between items-center mb-xs">
                          <div className="flex items-center gap-xs">
                            <span className="font-bold text-on-surface truncate">{c.display_name || c.peer_key_hex.substring(0, 16)}</span>
                            {c.is_favorite && <span className="material-symbols-outlined text-warning text-[16px]">star</span>}
                          </div>
                          <span className="text-label-xs text-on-surface-variant shrink-0">{formatTime(c.last_message_at ?? 0)}</span>
                        </div>
                        <p className="text-body-base text-on-surface-variant truncate">{c.last_message_preview || "No messages yet."}</p>
                      </div>
                      <div className="hidden group-hover:flex items-center gap-sm">
                        <button
                          className="text-on-surface-variant hover:text-danger transition-colors p-1 rounded-lg hover:bg-input-bg"
                          onClick={(e) => handleDeleteConversation(e, c.peer_key_hex)}
                          title="Delete conversation"
                        >
                          <span className="material-symbols-outlined text-[18px]">delete</span>
                        </button>
                      </div>
                    </div>
                  ))}

                {conversations.length === 0 && (
                  <div className="flex flex-col items-center justify-center py-4xl text-on-surface-variant gap-md">
                    <NoChatsIllustration />
                    <p className="font-headline-2xl text-headline-2xl text-on-surface">No conversations yet</p>
                    <p className="text-body-md max-w-[300px] text-center">Go to the Connect tab to create or join an encrypted session.</p>
                  </div>
                )}
              </div>
            </div>
          )}

          {/* ─── NEARBY TAB ─── */}
          {activeTab === "nearby" && (
            <div className="flex flex-col items-center justify-center py-4xl gap-lg">
              <RadarIllustration />
              <h2 className="font-headline-3xl text-headline-3xl text-on-surface">Nearby Discovery</h2>
              <p className="text-on-surface-variant text-center max-w-md text-body-md">Find and connect with peers securely over your local network using mDNS and DHT.</p>
            </div>
          )}

          {/* ─── FAMILY TAB ─── */}
          {activeTab === "family" && (
            <FamilyTab family={familyMembers} onRefresh={refreshFamily} onConnect={connectFamily} />
          )}
        </div>
      </div>

      {/* Footer */}
      <footer className="shrink-0 border-t border-border-subtle bg-surface/40 backdrop-blur-3xl px-xl py-md flex flex-col md:flex-row items-center justify-between gap-md">
        <div className="flex flex-col gap-xs">
          <span className="font-label-xs text-label-xs text-on-surface-variant uppercase tracking-[0.15em]">Identity Fingerprint</span>
          <div className="flex items-center gap-sm">
            <span className="font-mono-code text-mono-code text-secondary px-sm py-xs bg-input-bg rounded-lg border border-border-subtle text-[11px]">
              {identity?.fingerprint || "Loading..."}
            </span>
            <button
              onClick={() => {
                if (identity?.fingerprint) {
                  navigator.clipboard.writeText(identity.fingerprint);
                  addToast("Fingerprint copied!", "success");
                }
              }}
              className="text-on-surface-variant hover:text-primary transition-colors p-1 rounded-lg hover:bg-input-bg"
              title="Copy Fingerprint"
            >
              <span className="material-symbols-outlined text-[16px]">content_copy</span>
            </button>
          </div>
        </div>
        <div className="flex items-center gap-md">
          <span className="font-mono-label text-mono-label text-tertiary/60 bg-tertiary/5 px-2 py-0.5 rounded border border-tertiary/10 text-[10px]">Ed25519</span>
          <span className="font-mono-label text-mono-label text-primary/60 bg-primary/5 px-2 py-0.5 rounded border border-primary/10 text-[10px]">XChaCha20</span>
        </div>
      </footer>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </main>
  );
}
