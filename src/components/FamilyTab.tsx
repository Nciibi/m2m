import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useApp } from "../context/AppContext";
import type { FamilyMember } from "../types";

interface FamilyTabProps {
  family: FamilyMember[];
  onRefresh: () => Promise<void>;
  onConnect: (peerKeyHex: string) => Promise<void>;
}

export default function FamilyTab({ family, onRefresh, onConnect }: FamilyTabProps) {
  const { addToast } = useApp();
  const [showAdd, setShowAdd] = useState(false);
  const [showUpdate, setShowUpdate] = useState<string | null>(null);
  const [updateInvite, setUpdateInvite] = useState("");
  const [search, setSearch] = useState("");

  if (family.length === 0 && !showAdd) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-text-muted gap-4">
        <span className="material-symbols-outlined text-4xl opacity-50">family_restroom</span>
        <div className="text-center space-y-2">
          <p className="font-headline-2xl text-headline-2xl text-on-surface">No family members</p>
          <p className="font-body-md text-body-md max-w-[320px] mx-auto">Add people you trust to message them without generating an invite each time.</p>
        </div>
        <button onClick={() => setShowAdd(true)} className="flex items-center gap-2 px-md py-2 bg-primary-container text-on-primary-container rounded-xl hover:brightness-110 active:scale-95 transition-all mt-md">
          <span className="material-symbols-outlined text-[18px]">add</span>
          <span className="font-bold">Add to Family</span>
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full gap-lg">
      <div className="flex items-center justify-between shrink-0">
        <span className="text-on-surface-variant font-label-sm text-label-sm">{family.length} member{family.length !== 1 ? "s" : ""}</span>
        <button onClick={() => setShowAdd(true)} className="flex items-center gap-1 px-md py-1.5 bg-input-bg border border-outline-variant hover:bg-bg-hover/80 text-on-surface rounded-lg transition-all">
          <span className="material-symbols-outlined text-[16px]">add</span>
          <span className="font-bold text-sm">Add</span>
        </button>
      </div>

      {showAdd && <AddFamilyModal onClose={() => setShowAdd(false)} onDone={onRefresh} />}

      <div className="relative shrink-0">
        <span className="material-symbols-outlined absolute left-md top-1/2 -translate-y-1/2 text-text-muted text-[20px]">search</span>
        <input value={search} onChange={e => setSearch(e.target.value)} className="w-full h-[36px] bg-input-bg backdrop-blur-3xl border border-outline-variant rounded-lg pl-10 pr-md text-body-md text-text-primary focus:ring-1 focus:ring-primary outline-none transition-all" placeholder="Search family…" type="text"/>
      </div>

      <div className="flex-1 overflow-y-auto custom-scrollbar space-y-sm">
        {family.filter(m => m.nickname.toLowerCase().includes(search.toLowerCase()) || m.public_key_hex.includes(search)).map((m) => {
          const isExpired = m.expires_at !== null && m.expires_at * 1000 < Date.now();
          const daysLeft = m.expires_at ? Math.ceil((m.expires_at * 1000 - Date.now()) / 86400000) : null;

          return (
            <div key={m.public_key_hex} className="group inner-glass h-16 p-md rounded-xl flex items-center gap-md hover:bg-input-bg transition-all">
              <div className="relative flex-shrink-0">
                <div className="w-12 h-12 rounded-full bg-gradient-to-br from-[#10b981] to-[#3b82f6] flex items-center justify-center font-bold text-text-primary text-lg">
                  {m.nickname.charAt(0).toUpperCase()}
                </div>
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex justify-between items-center mb-xs">
                  <span className="font-bold text-text-primary truncate">{m.nickname}</span>
                  <span className={`text-label-xs ${isExpired ? "text-danger" : "text-tertiary"}`}>
                    {isExpired ? "Expired" : daysLeft !== null ? `${daysLeft}d left` : "Forever"}
                  </span>
                </div>
                <p className="text-body-base text-text-secondary truncate font-mono-code">{m.public_key_hex.substring(0, 16)}...</p>
              </div>
              
              <div className="flex items-center gap-sm">
                {isExpired ? (
                  <>
                    <button className="px-sm py-1 border border-outline-variant rounded-lg hover:bg-input-bg text-sm" onClick={() => { setShowUpdate(m.public_key_hex); setUpdateInvite(""); }}>Renew</button>
                    <button className="text-danger hover:bg-danger/10 p-1 rounded-lg" onClick={async () => {
                      try { await invoke("remove_family_member", { peerKeyHex: m.public_key_hex }); onRefresh(); } 
                      catch (e) { addToast("Failed to remove: " + e, "error"); }
                    }}><span className="material-symbols-outlined text-[18px]">delete</span></button>
                  </>
                ) : showUpdate === m.public_key_hex ? (
                  <div className="flex items-center gap-sm">
                    <input className="bg-input-bg border border-outline-variant rounded px-2 py-1 text-sm outline-none" placeholder="Paste new invite…" value={updateInvite} onChange={e => setUpdateInvite(e.target.value)} />
                    <button className="px-2 py-1 bg-primary text-on-primary rounded text-sm font-bold" disabled={!updateInvite} onClick={async () => {
                      try {
                        await invoke("update_family_member", { peerKeyHex: m.public_key_hex, inviteStr: updateInvite });
                        setShowUpdate(null); setUpdateInvite(""); onRefresh(); addToast("Family member updated", "success");
                      } catch (e) { addToast("Update failed: " + e, "error"); }
                    }}>Update</button>
                  </div>
                ) : (
                  <>
                    <button className="px-md py-1.5 bg-input-bg hover:bg-bg-hover/80 border border-outline-variant rounded-lg font-bold text-sm text-primary flex items-center gap-1 transition-all" onClick={async () => {
                      try { await onConnect(m.public_key_hex); } 
                      catch (e: any) { if (e?.toString().includes("CANNOT_REACH")) setShowUpdate(m.public_key_hex); }
                    }}>
                      <span className="material-symbols-outlined text-[16px]">chat</span> Chat
                    </button>
                    <button className="hidden group-hover:flex text-danger hover:bg-danger/10 p-1 rounded-lg" onClick={async () => {
                      try { await invoke("remove_family_member", { peerKeyHex: m.public_key_hex }); onRefresh(); } 
                      catch (e) { addToast("Failed to remove: " + e, "error"); }
                    }}><span className="material-symbols-outlined text-[18px]">delete</span></button>
                  </>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function AddFamilyModal({ onClose, onDone }: { onClose: () => void; onDone: () => Promise<void> }) {
  const { addToast } = useApp();
  const [peerKeyHex, setPeerKeyHex] = useState("");
  const [nickname, setNickname] = useState("");
  const [duration, setDuration] = useState("forever");
  const [customDays, setCustomDays] = useState("30");
  const [saving, setSaving] = useState(false);

  const handleSave = useCallback(async (e: React.MouseEvent | React.FormEvent) => {
    e.preventDefault();
    if (!peerKeyHex.trim() || !nickname.trim()) {
      addToast("Peer key and nickname are required", "warning");
      return;
    }
    setSaving(true);
    try {
      const expiresInDays = duration === "forever" ? null : duration === "custom" ? parseInt(customDays) : parseInt(duration);
      await invoke("add_family_member", {
        peerKeyHex: peerKeyHex.trim(),
        nickname: nickname.trim(),
        expiresInDays: expiresInDays || null,
      });
      onClose();
      await onDone();
      addToast("Added to family", "success");
    } catch (e) {
      addToast("Failed to add: " + e, "error");
    } finally {
      setSaving(false);
    }
  }, [peerKeyHex, nickname, duration, customDays, onClose, onDone, addToast]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm">
      <div className="bg-surface border border-outline-variant rounded-2xl p-xl shadow-2xl max-w-md w-full animate-in fade-in zoom-in-95 duration-200">
        <div className="flex justify-between items-center mb-xl">
          <h2 className="font-headline-2xl text-headline-2xl font-bold">Add to Family</h2>
          <button onClick={onClose} className="text-text-muted hover:text-text-primary transition-colors p-1 rounded-lg hover:bg-input-bg"><span className="material-symbols-outlined">close</span></button>
        </div>
        
        <div className="space-y-md">
          <div className="space-y-xs">
            <label className="text-label-sm text-text-muted uppercase tracking-wider">Peer Key</label>
            <input className="w-full bg-input-bg border border-outline-variant rounded-lg px-md py-2 font-mono-code focus:ring-1 focus:ring-primary outline-none" placeholder="Peer public key hex" value={peerKeyHex} onChange={e => setPeerKeyHex(e.target.value)} />
          </div>
          <div className="space-y-xs">
            <label className="text-label-sm text-text-muted uppercase tracking-wider">Nickname</label>
            <input className="w-full bg-input-bg border border-outline-variant rounded-lg px-md py-2 focus:ring-1 focus:ring-primary outline-none" placeholder="How you'll know them" value={nickname} onChange={e => setNickname(e.target.value)} />
          </div>
          <div className="space-y-xs">
            <label className="text-label-sm text-text-muted uppercase tracking-wider">Duration</label>
            <select className="w-full bg-input-bg border border-outline-variant rounded-lg px-md py-2 focus:ring-1 focus:ring-primary outline-none" value={duration} onChange={e => setDuration(e.target.value)}>
              <option value="forever">Forever</option>
              <option value="7">7 days</option>
              <option value="30">30 days</option>
              <option value="90">90 days</option>
              <option value="custom">Custom</option>
            </select>
          </div>
          {duration === "custom" && (
            <div className="space-y-xs animate-in slide-in-from-top-2 duration-200">
              <label className="text-label-sm text-text-muted uppercase tracking-wider">Days</label>
              <input type="number" min={1} className="w-full bg-input-bg border border-outline-variant rounded-lg px-md py-2 focus:ring-1 focus:ring-primary outline-none" value={customDays} onChange={e => setCustomDays(e.target.value)} />
            </div>
          )}
        </div>

        <div className="flex justify-end gap-md mt-xl pt-md border-t border-outline-variant">
          <button onClick={onClose} className="px-md py-2 rounded-lg font-bold hover:bg-input-bg transition-colors">Cancel</button>
          <button onClick={handleSave} disabled={saving} className="px-lg py-2 rounded-lg font-bold bg-primary-container text-on-primary-container hover:brightness-110 active:scale-95 transition-all disabled:opacity-50 flex items-center gap-2">
            {saving ? <span className="material-symbols-outlined animate-spin text-[18px]">sync</span> : "Add Member"}
          </button>
        </div>
      </div>
    </div>
  );
}
