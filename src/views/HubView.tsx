import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ToastContainer } from "../components/ui";
import { useApp } from "../context/AppContext";
import { useChat } from "../context/ChatContext";
import FamilyTab from "../components/FamilyTab";
import type { FamilyMember } from "../types";
import { formatTime } from "../utils";

export default function HubView() {
  const { identity, toasts, removeToast, setView } = useApp();
  const { conversations, handleOpenChat: ctxHandleOpenChat } = useChat();
  const [activeTab, setActiveTab] = useState<"connect" | "chats" | "nearby" | "family">("connect");
  const [familyMembers, setFamilyMembers] = useState<FamilyMember[]>([]);

  const refreshFamily = async () => {
    try { setFamilyMembers(await invoke<FamilyMember[]>("get_family_members")); } catch {}
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
  const [copied, setCopied] = useState(false);
  const inviteValid = inviteToConnect.startsWith("m2m://") && inviteToConnect.length > 20;

  // Search for Chats
  const [search, setSearch] = useState("");

  const handleGenerateInvite = async () => {
    try {
      const invite = await invoke<string>("generate_invite_link");
      setGeneratedInvite(invite);
    } catch (e) {}
  };

  const handleCopyInvite = () => {
    if (generatedInvite) {
      navigator.clipboard.writeText(generatedInvite);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const handleConnect = async () => {
    if (!inviteValid) return;
    setIsConnecting(true);
    try {
      await invoke("join_invite_link", { link: inviteToConnect });
      if (namingTheirName || namingMyName) {
        try {
          const peerKey = inviteToConnect.split("//")[1]?.split("/")[0] || "";
          if (peerKey) {
            await invoke("update_conversation_name", { peerKeyHex: peerKey, name: namingTheirName });
          }
        } catch {}
      }
      setView("chat");
    } catch (e) {
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

  return (
    <main className="glass-panel w-full max-w-container-max h-[100dvh] md:h-[962px] md:my-auto md:rounded-[32px] flex flex-col relative z-10 overflow-hidden mx-auto shadow-[0_0_50px_-12px_rgba(0,0,0,0.8)] border border-white/5 bg-surface/60 backdrop-blur-[60px] saturate-[1.2]">
      {/* Header */}
      <header className="h-[64px] px-xl flex items-center justify-between border-b border-border-subtle shrink-0 bg-surface/80 backdrop-blur-3xl">
        <div className="flex items-center gap-md">
          <div className="w-8 h-8 rounded-lg bg-gradient-to-tr from-primary-container to-inverse-primary flex items-center justify-center shadow-lg shadow-primary-container/20">
            <span className="material-symbols-outlined text-white text-[20px]" style={{ fontVariationSettings: "'FILL' 1" }}>security</span>
          </div>
          <span className="font-headline-2xl text-headline-2xl font-extrabold tracking-tight text-on-surface">M2M</span>
        </div>
        <div className="flex items-center gap-lg">
          <div className="flex items-center gap-sm bg-surface-container-low/50 px-md py-xs rounded-full border border-border-subtle">
            <div className="w-2 h-2 rounded-full bg-tertiary-fixed-dim status-glow animate-pulse"></div>
            <span className="font-label-sm text-label-sm text-on-surface">Online</span>
          </div>
          <button onClick={() => setView("settings")} className="text-on-surface-variant hover:text-primary transition-colors active:scale-95">
            <span className="material-symbols-outlined text-[20px]">settings</span>
          </button>
        </div>
      </header>

      {/* Tab Bar */}
      <nav className="h-[44px] px-xl flex items-center border-b border-border-subtle bg-surface-container-lowest/30 shrink-0">
        <div className="flex items-center h-full gap-xl">
          <button onClick={() => setActiveTab("connect")} className={`h-full flex items-center gap-sm px-xs border-b-2 transition-all ${activeTab === "connect" ? "border-primary text-on-surface" : "border-transparent text-on-surface-variant hover:text-on-surface group"}`}>
            <span className="material-symbols-outlined text-[18px]">link</span>
            <span className={`font-label-sm text-label-sm ${activeTab === "connect" ? "font-bold" : ""}`}>Connect</span>
          </button>
          <button onClick={() => setActiveTab("chats")} className={`h-full flex items-center gap-sm px-xs border-b-2 transition-all ${activeTab === "chats" ? "border-primary text-on-surface" : "border-transparent text-on-surface-variant hover:text-on-surface group"}`}>
            <span className="material-symbols-outlined text-[18px]">chat_bubble</span>
            <span className={`font-label-sm text-label-sm ${activeTab === "chats" ? "font-bold" : ""}`}>Chats</span>
            {conversations.length > 0 && <span className="bg-primary-container text-on-primary-container text-[10px] px-1.5 py-0.5 rounded-full font-bold">{conversations.length}</span>}
          </button>
          <button onClick={() => setActiveTab("nearby")} className={`h-full flex items-center gap-sm px-xs border-b-2 transition-all ${activeTab === "nearby" ? "border-primary text-on-surface" : "border-transparent text-on-surface-variant hover:text-on-surface group"}`}>
            <span className="material-symbols-outlined text-[18px]">wifi</span>
            <span className={`font-label-sm text-label-sm ${activeTab === "nearby" ? "font-bold" : ""}`}>Nearby</span>
          </button>
          <button onClick={() => setActiveTab("family")} className={`h-full flex items-center gap-sm px-xs border-b-2 transition-all ${activeTab === "family" ? "border-primary text-on-surface" : "border-transparent text-on-surface-variant hover:text-on-surface group"}`}>
            <span className="material-symbols-outlined text-[18px]">group</span>
            <span className={`font-label-sm text-label-sm ${activeTab === "family" ? "font-bold" : ""}`}>Family</span>
          </button>
        </div>
      </nav>

      {/* Main Content Area */}
      <div className="flex-1 p-xl overflow-y-auto space-y-xl custom-scrollbar flex flex-col">
        {activeTab === "connect" && (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-xl h-full">
            {/* Host Connection */}
            <section className="glass-card rounded-2xl p-xl flex flex-col h-full">
              <div className="flex items-center gap-md mb-xl">
                <div className="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center border border-primary/20">
                  <span className="material-symbols-outlined text-primary">broadcast_on_personal</span>
                </div>
                <div>
                  <h2 className="font-headline-2xl text-headline-2xl text-on-surface">Host a Connection</h2>
                  <p className="font-body-md text-body-md text-on-surface-variant">Create a secure link for others to join.</p>
                </div>
              </div>
              <button onClick={handleGenerateInvite} className="w-full py-md px-xl bg-gradient-to-r from-primary-container to-inverse-primary text-on-primary-container rounded-xl font-headline-2xl text-headline-2xl font-bold flex items-center justify-center gap-md hover:brightness-125 active:scale-[0.98] transition-all duration-300 shadow-[0_0_20px_rgba(99,102,241,0.2)] hover:shadow-[0_0_30px_rgba(99,102,241,0.5)] mb-xl border border-white/10 group">
                <span className="material-symbols-outlined group-hover:rotate-12 transition-transform duration-300">{generatedInvite ? "refresh" : "add_link"}</span>
                {generatedInvite ? "Regenerate Invite Link" : "Generate Invite Link"}
              </button>
              {generatedInvite && (
                <div className="space-y-md">
                  <label className="font-label-sm text-label-sm text-on-surface-variant uppercase tracking-wider">Your Active Invite</label>
                  <div className="flex items-center gap-sm bg-input-bg p-md rounded-xl border border-border-subtle group">
                    <span className="font-mono-code text-mono-code text-primary flex-1 break-all overflow-hidden whitespace-nowrap">{generatedInvite}</span>
                    <button onClick={handleCopyInvite} className="text-on-surface-variant hover:text-primary transition-colors p-sm rounded-lg hover:bg-white/5">
                      <span className="material-symbols-outlined text-[20px]">{copied ? "check" : "content_copy"}</span>
                    </button>
                  </div>
                </div>
              )}
            </section>

            {/* Join Connection */}
            <section className="glass-card rounded-2xl p-xl flex flex-col h-full">
              <div className="flex items-center gap-md mb-xl">
                <div className="w-10 h-10 rounded-xl bg-tertiary/10 flex items-center justify-center border border-tertiary/20">
                  <span className="material-symbols-outlined text-tertiary">key</span>
                </div>
                <div>
                  <h2 className="font-headline-2xl text-headline-2xl text-on-surface">Join a Connection</h2>
                  <p className="font-body-md text-body-md text-on-surface-variant">Enter a link to start an encrypted chat.</p>
                </div>
              </div>
              <div className="space-y-xl">
                <div className="space-y-md">
                  <label className="font-label-sm text-label-sm text-on-surface-variant uppercase tracking-wider">Invite Link</label>
                  <input value={inviteToConnect} onChange={e => setInviteToConnect(e.target.value)} className="w-full bg-input-bg border border-border-subtle rounded-xl px-xl py-md text-primary font-mono-code focus:ring-1 focus:ring-primary focus:border-primary placeholder:text-outline-variant outline-none transition-all" placeholder="m2m://..." type="text"/>
                  {inviteValid && (
                    <div className="flex items-center gap-sm text-tertiary font-label-sm text-label-sm px-xs">
                      <span className="material-symbols-outlined text-[16px]">check_circle</span>
                      <span>Valid Invite Found</span>
                    </div>
                  )}
                </div>
                <div className="grid grid-cols-2 gap-lg">
                  <div className="space-y-md">
                    <label className="font-label-sm text-label-sm text-on-surface-variant uppercase tracking-wider">Your Name (Optional)</label>
                    <input value={namingMyName} onChange={e => setNamingMyName(e.target.value)} className="w-full bg-input-bg border border-border-subtle rounded-xl px-lg py-md text-on-surface focus:ring-1 focus:ring-primary focus:border-primary outline-none" placeholder="Nexus-01" type="text"/>
                  </div>
                  <div className="space-y-md">
                    <label className="font-label-sm text-label-sm text-on-surface-variant uppercase tracking-wider">Their Name (Optional)</label>
                    <input value={namingTheirName} onChange={e => setNamingTheirName(e.target.value)} className="w-full bg-input-bg border border-border-subtle rounded-xl px-lg py-md text-on-surface focus:ring-1 focus:ring-primary focus:border-primary outline-none" placeholder="Ghost-Host" type="text"/>
                  </div>
                </div>
                <button onClick={handleConnect} disabled={!inviteValid || isConnecting} className="w-full py-md px-xl bg-gradient-to-r from-tertiary-container to-tertiary text-on-tertiary-container rounded-xl font-headline-2xl text-headline-2xl font-bold flex items-center justify-center gap-md hover:brightness-125 active:scale-[0.98] transition-all duration-300 shadow-[0_0_20px_rgba(16,185,129,0.2)] hover:shadow-[0_0_30px_rgba(16,185,129,0.5)] border border-white/10 disabled:opacity-50 disabled:cursor-not-allowed group">
                  <span className={`material-symbols-outlined ${isConnecting ? 'animate-spin' : 'group-hover:scale-110 transition-transform duration-300'}`}>{isConnecting ? "sync" : "sensors"}</span>
                  {isConnecting ? "Connecting..." : "Connect"}
                </button>
              </div>
              {/* Atmospheric Graphic Element */}
              <div className="mt-auto pt-4xl flex justify-center">
                <div className="relative w-32 h-32 flex items-center justify-center opacity-30">
                  <div className="absolute inset-0 border border-primary/20 rounded-full animate-ping"></div>
                  <div className="absolute inset-4 border border-tertiary/20 rounded-full animate-[ping_2s_infinite]"></div>
                  <span className="material-symbols-outlined text-primary/40 text-4xl">vpn_lock</span>
                </div>
              </div>
            </section>
          </div>
        )}

        {activeTab === "chats" && (
          <div className="flex-1 flex flex-col max-w-3xl mx-auto w-full gap-lg overflow-hidden h-full">
            {/* Search Bar */}
            <div className="relative shrink-0">
              <span className="material-symbols-outlined absolute left-md top-1/2 -translate-y-1/2 text-text-muted text-[20px]">search</span>
              <input value={search} onChange={e => setSearch(e.target.value)} className="w-full h-[36px] bg-black/40 backdrop-blur-3xl border border-white/10 rounded-lg pl-10 pr-md text-body-md text-white focus:ring-1 focus:ring-primary/50 outline-none transition-all" placeholder="Search conversations…" type="text"/>
            </div>
            {/* Conversation List */}
            <div className="flex-1 overflow-y-auto custom-scrollbar space-y-sm">
              {conversations.filter(c => c.peer_key_hex.includes(search) || (c.display_name && c.display_name.toLowerCase().includes(search.toLowerCase()))).map((c, i) => (
                <div key={c.peer_key_hex} onClick={() => handleOpenChat(c.peer_key_hex)} className="group inner-glass h-16 p-md rounded-xl flex items-center gap-md cursor-pointer hover:bg-white/10 hover:border-white/20 transition-all duration-300 hover:scale-[1.01] hover:shadow-lg animate-in slide-in-from-bottom-2 fade-in" style={{ animationDelay: `${i * 50}ms`, animationFillMode: "both" }}>
                  <div className="relative flex-shrink-0">
                    <div className="w-12 h-12 rounded-full bg-gradient-to-br from-[#6366f1] to-[#a855f7] flex items-center justify-center font-bold text-white text-lg">
                      {(c.display_name || c.peer_key_hex).charAt(0).toUpperCase()}
                    </div>
                    {c.is_online && <div className="absolute top-0 right-0 w-3 h-3 bg-tertiary border-2 border-[#030408] rounded-full status-dot"></div>}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex justify-between items-center mb-xs">
                      <div className="flex items-center gap-xs">
                        <span className="font-bold text-white truncate">{c.display_name || c.peer_key_hex.substring(0, 16)}</span>
                        {c.is_favorite && <span className="material-symbols-outlined text-warning text-[16px]">star</span>}
                      </div>
                      <span className="text-label-xs text-text-muted">{formatTime(c.last_message_at ?? 0)}</span>
                    </div>
                    <p className="text-body-base text-secondary truncate">{c.last_message_preview || "No messages yet."}</p>
                  </div>
                  <div className="hidden group-hover:flex items-center gap-sm text-text-muted">
                    <span className="material-symbols-outlined text-[18px] hover:text-danger" onClick={(e) => { e.stopPropagation(); invoke("delete_conversation", { peerKeyHex: c.peer_key_hex }); }}>delete</span>
                  </div>
                </div>
              ))}
              {conversations.length === 0 && (
                <div className="flex flex-col items-center justify-center h-full text-text-muted">
                  <span className="material-symbols-outlined text-4xl mb-4 opacity-50">chat_bubble_outline</span>
                  <p>No conversations yet.</p>
                </div>
              )}
            </div>
          </div>
        )}

        {activeTab === "nearby" && (
          <div className="flex-1 flex flex-col items-center justify-center max-w-3xl mx-auto w-full gap-lg overflow-hidden h-full">
            <span className="material-symbols-outlined text-[64px] text-primary/50 mb-4">wifi_tethering</span>
            <h2 className="font-headline-3xl text-headline-3xl text-on-surface">Nearby Discovery</h2>
            <p className="text-on-surface-variant text-center max-w-md">Find and connect with peers securely over your local network using mDNS and DHT.</p>
          </div>
        )}

        {activeTab === "family" && (
          <div className="flex-1 max-w-3xl mx-auto w-full h-full overflow-y-auto custom-scrollbar">
             <FamilyTab family={familyMembers} onRefresh={refreshFamily} onConnect={connectFamily} />
          </div>
        )}
      </div>

      {/* Footer */}
      <footer className="shrink-0 border-t border-border-subtle bg-surface-container-lowest/50 p-xl flex flex-col md:flex-row items-center justify-between gap-xl">
        <div className="flex flex-col gap-sm">
          <span className="font-label-xs text-label-xs text-on-surface-variant uppercase tracking-[0.2em]">Your Identity Fingerprint</span>
          <div className="flex items-center gap-md">
            <span className="font-mono-code text-mono-code text-secondary px-md py-xs bg-white/5 rounded-lg border border-border-subtle">
              {identity?.fingerprint || "Loading fingerprint..."}
            </span>
            <button onClick={() => { if(identity?.fingerprint) { navigator.clipboard.writeText(identity.fingerprint); setCopied(true); setTimeout(() => setCopied(false), 2000); } }} className="text-on-surface-variant hover:text-primary transition-colors p-sm rounded-lg hover:bg-white/5 active:scale-90" title="Copy Fingerprint">
              <span className="material-symbols-outlined text-[18px]">{copied ? "check" : "content_copy"}</span>
            </button>
          </div>
        </div>
        <div className="flex items-center gap-4xl">
          <div className="flex flex-col items-end">
            <span className="font-label-xs text-label-xs text-on-surface-variant uppercase tracking-[0.2em]">End-to-End Encryption</span>
            <div className="flex items-center gap-sm mt-1">
              <span className="font-mono-label text-mono-label text-tertiary bg-tertiary/10 px-2 py-0.5 rounded border border-tertiary/20">Ed25519</span>
              <span className="font-mono-label text-mono-label text-primary bg-primary/10 px-2 py-0.5 rounded border border-primary/20">XChaCha20-Poly1305</span>
            </div>
          </div>
        </div>
      </footer>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </main>
  );
}
