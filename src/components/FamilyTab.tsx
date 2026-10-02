import { useNow } from "../hooks/useNow";
import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, Input, Modal } from "./ui";
// `ConfirmDialog` is not in the `./ui` barrel — `SettingsView` imports it from
// the module directly too.
import { ConfirmDialog } from "./ui/ConfirmDialog";
import { PlusIcon, AlertTriangleIcon } from "./ui/Icons";
import { useApp } from "../context/AppContext";
import type { FamilyMember } from "../types";
import { errorMessage, hashToColor } from "../utils";
import { asAppError } from "../events";

interface FamilyTabProps {
  family: FamilyMember[];
  onRefresh: () => Promise<void>;
  onConnect: (peerKeyHex: string) => Promise<void>;
}

export default function FamilyTab({ family, onRefresh, onConnect, loadError = null, loading = false }: FamilyTabProps) {
  const { addToast } = useApp();
  const [showAdd, setShowAdd] = useState(false);
  const [showUpdate, setShowUpdate] = useState<string | null>(null);
  const [updateInvite, setUpdateInvite] = useState("");
  // Which member is pending removal, for the confirmation dialog.
  const [pendingRemoval, setPendingRemoval] = useState<FamilyMember | null>(null);
  // Day-level granularity is plenty for an expiry badge, and it keeps a long
  // family list from re-rendering every second.
  const now = useNow(60_000);

  // Read failed. Distinct from "no members": rendering the empty state here
  // would tell the user every family member had been removed, and invite them
  // to re-add people who are already on the list.
  if (loadError) {
    return (
      <div className="conv-empty">
        <AlertTriangleIcon size={48} color="var(--color-danger)" />
        <span style={{ fontSize: "var(--text-lg)", fontWeight: 600, color: "var(--color-text-primary)" }}>
          Could not load your family
        </span>
        <span style={{ maxWidth: "320px", textAlign: "center", lineHeight: 1.6 }}>
          {loadError}
        </span>
        <Button onClick={onRefresh} style={{ marginTop: "var(--space-md)" }}>
          Retry
        </Button>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="conv-empty">
        <span style={{ color: "var(--color-text-muted)" }}>Loading family…</span>
      </div>
    );
  }

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
        // `now` comes from useNow(): reading Date.now() inline here was impure
        // and never re-evaluated, so "expires in N days" could sit stale for the
        // lifetime of the window.
        const isExpired = m.expires_at !== null && m.expires_at * 1000 < now;
        const daysLeft = m.expires_at ? Math.ceil((m.expires_at * 1000 - now) / 86400000) : null;

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
                  <Button
                    size="xs"
                    variant="secondary"
                    aria-label={`Remove ${m.nickname} from family`}
                    onClick={() => setPendingRemoval(m)}
                  >×</Button>
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
                      addToast("Update failed: " + errorMessage(e), "error");
                    }
                  }}>Update</Button>
                </div>
              ) : (
                <>
                  <Button size="xs" onClick={async () => {
                    try {
                      await onConnect(m.public_key_hex);
                    } catch (e) {
                      // Branch on the code. This used to substring-match
                      // `String(e).includes("CANNOT_REACH")`, which cannot match
                      // any more: the rejection is now `{code, message}`, so
                      // `String(e)` is `"[object Object]"`. It also swallowed
                      // every *other* failure silently — connecting to a family
                      // member who was simply offline showed the user nothing.
                      if (asAppError(e)?.code === "family.unreachable") {
                        setShowUpdate(m.public_key_hex);
                      } else {
                        addToast(errorMessage(e), "error");
                      }
                    }
                  }}>Msg</Button>
                  <Button
                    size="xs"
                    variant="secondary"
                    aria-label={`Remove ${m.nickname} from family`}
                    onClick={() => setPendingRemoval(m)}
                  >×</Button>
                </>
              )}
            </div>
          </div>
        );
      })}

    {/* Removing a family member revokes their standing trust — they can no
          longer be reached without a fresh invite. It used to be a bare `×`
          with no accessible name and no confirmation: a screen reader announced
          "multiplication sign", and one misclick silently dropped someone out
          of the trust list. */}
      {pendingRemoval && (
        <ConfirmDialog
          open
          destructive
          title="Remove family member"
          body={`Remove ${pendingRemoval.nickname} from your family? They will need a fresh invite before you can message them again.`}
          confirmLabel="Remove"
          cancelLabel="Cancel"
          onCancel={() => setPendingRemoval(null)}
          onConfirm={async () => {
            const target = pendingRemoval;
            setPendingRemoval(null);
            try {
              await invoke("remove_family_member", { peerKeyHex: target.public_key_hex });
              onRefresh();
              addToast(`Removed ${target.nickname}`, "success");
            } catch (e) {
              addToast("Failed to remove: " + errorMessage(e), "error");
            }
          }}
        />
      )}
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
      addToast("Failed to add: " + errorMessage(e), "error");
    } finally {
      setSaving(false);
    }
  }, [peerKeyHex, nickname, duration, customDays, onClose, onDone, addToast]);

  return (
    <Modal open={true} title="Add to Family" onClose={onClose}>
      <div className="modal-form">
        <label>
          Peer Key
          <Input
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

