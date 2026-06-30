import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, Input, Modal } from "./ui";
import { PlusIcon, AlertTriangleIcon } from "./ui/Icons";
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

  if (family.length === 0) {
    return (
      <div className="conv-empty">
        <AlertTriangleIcon size={48} color="var(--color-text-muted)" />
        <span style={{ fontSize: "var(--text-lg)", fontWeight: 600, color: "var(--color-text-primary)" }}>
          No family members
        </span>
        <span style={{ maxWidth: "320px", textAlign: "center", lineHeight: 1.6 }}>
          Add people you trust to message them without generating an invite each time.
        </span>
        <Button onClick={() => setShowAdd(true)} icon={<PlusIcon size={18} />} style={{ marginTop: "var(--space-md)" }}>
          Add to Family
        </Button>
        {showAdd && <AddFamilyModal onClose={() => setShowAdd(false)} onDone={onRefresh} />}
      </div>
    );
  }

  return (
    <div className="conv-list">
      <div className="family-header">
        <span className="text-muted text-sm">{family.length} member{family.length !== 1 ? "s" : ""}</span>
        <Button size="xs" onClick={() => setShowAdd(true)} icon={<PlusIcon size={14} />}>Add</Button>
      </div>

      {showAdd && <AddFamilyModal onClose={() => setShowAdd(false)} onDone={onRefresh} />}

      {family.map((m) => {
        const isExpired = m.expires_at !== null && m.expires_at * 1000 < Date.now();
        const daysLeft = m.expires_at ? Math.ceil((m.expires_at * 1000 - Date.now()) / 86400000) : null;

        return (
          <div key={m.public_key_hex} className="conv-item">
            <div className="conv-avatar conv-avatar--online" style={{
              background: `linear-gradient(135deg, ${hashToColor(m.public_key_hex)}, ${hashToColor(m.public_key_hex.slice(16))})`,
            }}>
              {m.nickname.charAt(0).toUpperCase()}
            </div>
            <div className="conv-body">
              <div className="conv-top">
                <span className="conv-name">{m.nickname}</span>
              </div>
              <span className="conv-preview">
                {isExpired ? "Expired" : daysLeft !== null ? `${daysLeft}d left` : "Forever"}
                {m.last_address ? ` · ${m.last_address}` : ""}
              </span>
            </div>
            <div className="conv-actions">
              {isExpired ? (
                <>
                  <Button size="xs" variant="secondary" onClick={() => { setShowUpdate(m.public_key_hex); setUpdateInvite(""); }}>
                    Renew
                  </Button>
                  <Button size="xs" variant="secondary" onClick={async () => {
                    try {
                      await invoke("remove_family_member", { peerKeyHex: m.public_key_hex });
                      onRefresh();
                    } catch (e) {
                      addToast("Failed to remove: " + e, "error");
                    }
                  }}>×</Button>
                </>
              ) : showUpdate === m.public_key_hex ? (
                <div className="flex-row" style={{ gap: "var(--space-xs)" }}>
                  <Input
                    compact
                    placeholder="Paste new invite…"
                    value={updateInvite}
                    onChange={e => setUpdateInvite(e.target.value)}
                  />
                  <Button size="xs" disabled={!updateInvite} onClick={async () => {
                    try {
                      await invoke("update_family_member", { peerKeyHex: m.public_key_hex, inviteStr: updateInvite });
                      setShowUpdate(null);
                      setUpdateInvite("");
                      onRefresh();
                      addToast("Family member updated", "success");
                    } catch (e) {
                      addToast("Update failed: " + e, "error");
                    }
                  }}>Update</Button>
                </div>
              ) : (
                <>
                  <Button size="xs" onClick={async () => {
                    try {
                      await onConnect(m.public_key_hex);
                    } catch (e: any) {
                      if (e?.toString().includes("CANNOT_REACH")) {
                        setShowUpdate(m.public_key_hex);
                      }
                    }
                  }}>Msg</Button>
                  <Button size="xs" variant="secondary" onClick={async () => {
                    try {
                      await invoke("remove_family_member", { peerKeyHex: m.public_key_hex });
                      onRefresh();
                    } catch (e) {
                      addToast("Failed to remove: " + e, "error");
                    }
                  }}>×</Button>
                </>
              )}
            </div>
          </div>
        );
      })}

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

  const handleSave = useCallback(async () => {
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
    <Modal open={true} title="Add to Family" onClose={onClose}>
      <div className="modal-form">
        <label>
          Peer Key
          <datalist id="peer-list">
          <Input
            list="peer-list"
            placeholder="Peer public key hex"
            value={peerKeyHex}
            onChange={e => setPeerKeyHex(e.target.value)}
            mono
          />
        </label>
        <label>
          Nickname
          <Input placeholder="How you'll know them" value={nickname} onChange={e => setNickname(e.target.value)} />
        </label>
        <label>
          Duration
          <select className="select" value={duration} onChange={e => setDuration(e.target.value)}>
            <option value="forever">Forever</option>
            <option value="7">7 days</option>
            <option value="30">30 days</option>
            <option value="90">90 days</option>
            <option value="custom">Custom</option>
          </select>
        </label>
        {duration === "custom" && (
          <label>
            Days
            <Input type="number" min={1} value={customDays} onChange={e => setCustomDays(e.target.value)} />
          </label>
        )}
        <div className="flex-row" style={{ justifyContent: "flex-end", gap: "var(--space-sm)", marginTop: "var(--space-md)" }}>
          <Button variant="secondary" onClick={onClose}>Cancel</Button>
          <Button onClick={handleSave} loading={saving} disabled={saving}>Add</Button>
        </div>
      </div>
    </Modal>
  );
}

function hashToColor(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) hash = str.charCodeAt(i) + ((hash << 5) - hash);
  return `hsl(${Math.abs(hash) % 360}, 55%, 48%)`;
}
