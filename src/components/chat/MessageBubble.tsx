import { useState, useEffect, useRef, useCallback } from "react";
import { Button } from "../ui";
import { SmileyIcon, ChevronDownIcon, CheckDoubleIcon, ClockIcon } from "../ui/Icons";
import SelfDestructTimer from "./SelfDestructTimer";
import { renderMarkdown } from "./messageRender";
import type { ChatMessage } from "../../types";

export interface MessageBubbleProps {
  message: ChatMessage;
  index?: number;
  /** Render content as plain text (no markdown) */
  plain?: boolean;
  msgStatus?: "sending" | "sent" | "delivered" | "read";
  onReact?: (messageId: string, emoji: string) => void;
  onRemoveReaction?: (messageId: string, emoji: string) => void;
  onEditSave?: (messageId: string, content: string) => Promise<void> | void;
  onDelete?: (messageId: string) => Promise<void> | void;
  /**
   * Our own Ed25519 public key, hex. Used to decide whether a reaction chip is
   * ours.
   *
   * A prop rather than `useApp()` so this component stays renderable in
   * isolation — the whole test suite mounts it bare, and a context read would
   * force a provider onto every one of those tests for no behavioural gain.
   */
  myPeerKeyHex?: string;
}

const PICKER_EMOJIS = ["👍", "❤️", "😂", "😮", "😢", "🙏"];

export default function MessageBubble({
  message: m,
  index = 0,
  plain = false,
  msgStatus,
  onReact,
  onRemoveReaction,
  onEditSave,
  onDelete,
  myPeerKeyHex,
}: MessageBubbleProps) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [editing, setEditing] = useState(false);
  const [editText, setEditText] = useState("");
  // Records that the currently-visible picker was opened by hover rather than by
  // the click handler. A real click is preceded by a mouseenter, so without
  // this the click's `!pickerOpen` toggle immediately closed the picker that
  // hover had just opened — the button did nothing for mouse users.
  const hoverOpenedRef = useRef(false);

  // Close context menu on click outside
  useEffect(() => {
    if (!menuOpen) return;
    const handler = () => setMenuOpen(false);
    window.addEventListener("click", handler, { once: true });
    return () => window.removeEventListener("click", handler);
  }, [menuOpen]);

  // Only treat a *failed* edit save as a failure.
  //
  // `await onEditSave?.(...)` then `setEditing(false)` unconditionally meant a
  // rejected save closed the editor and discarded the user's retyped text with
  // no message — indistinguishable from a successful edit. The error itself is
  // reported by the caller (`handleEditMessage` toasts it), so here we only
  // need to keep the editor open when the promise rejects.
  const saveEdit = useCallback(async () => {
    if (!onEditSave) return;
    try {
      await onEditSave(m.id, editText);
      setEditing(false);
    } catch {
      // Editor stays open with the text intact. The caller has already told
      // the user why.
    }
  }, [onEditSave, m.id, editText]);

  const canEdit = typeof onEditSave === "function";
  const canDelete = typeof onDelete === "function";
  const canReact = typeof onReact === "function" || typeof onRemoveReaction === "function";
  const senderLabel = m.direction === "sent" ? "you" : m.sender_peer_key_hex ? m.sender_peer_key_hex.substring(0, 8) : "peer";

  // "Did I react to this?" — by comparing against our OWN key.
  //
  // This used to test `reactors.includes("self")`, a sentinel the context wrote
  // optimistically. But the backend persists the real Ed25519 key
  // (`send_reaction` → `upsert_reaction(..., &peer_key_hex, ...)`), so
  // `load_messages` returns `[<64-hex>]` and never `"self"`. After a reload the
  // chip rendered unhighlighted with `aria-pressed="false"` and clicking it
  // *added* the reaction again instead of removing it — so a user could add
  // their own reaction and never take it back.
  //
  // It also violated this app's own event boundary: `events.ts` validates every
  // reactor with `isPeerKeyHex`, so `"self"` is a value the validator would
  // reject. Both sides now speak the persisted shape. The `"self"` case is still
  // accepted as a fallback so an unrecognised or absent `myPeerKeyHex` degrades
  // to the old behaviour instead of treating the user's own reaction as
  // someone else's and letting them add it twice.
  const reactedByMe = (reactors: string[]) =>
    (!!myPeerKeyHex && reactors.includes(myPeerKeyHex)) || reactors.includes("self");

  return (
    <div
      className={`msg-bubble msg-bubble--${m.direction}${m.deleted ? " msg-bubble--deleted" : ""}`}
      style={{ animationDelay: `${index * 0.05}s` }}
      // NOT a tab stop. It used to be `tabIndex={0}`, so reaching the message
      // composer in a long conversation meant tabbing through every preceding
      // bubble — each of which announced the first 40 characters of its
      // content aloud. In a 500-message thread that is 500 stops and a wall of
      // speech, which makes the app effectively unusable by keyboard or screen
      // reader.
      //
      // The transcript is now a `role="log"` region (see ChatView), which
      // assistive tech reads as a live stream without requiring focus, so the
      // message text stays reachable without 500 tab stops. The interactive
      // affordances (react / edit / delete) are focusable in their own right.
      role="group"
      aria-label={`Message from ${senderLabel}`}
      onMouseEnter={() => { if (canReact) { hoverOpenedRef.current = true; setPickerOpen(true); } }}
      onMouseLeave={() => { hoverOpenedRef.current = false; setPickerOpen(false); }}
      onContextMenu={(e) => { if (!canEdit && !canDelete) return; e.preventDefault(); setMenuOpen(true); }}
      onKeyDown={(e) => { if (e.key === "Escape") { if (pickerOpen) { setPickerOpen(false); e.stopPropagation(); } if (menuOpen) { setMenuOpen(false); e.stopPropagation(); } } }}
    >
      {!m.deleted && canReact && (
        <button type="button" className="msg-bubble-action msg-bubble-action--react"
          aria-label="Toggle reaction picker" aria-expanded={pickerOpen}
          onClick={(e) => {
            e.stopPropagation();
            // Keyboard and touch never fire mouseenter, so the toggle is the
            // only thing that opens the picker there. When hover *did* open
            // it, this click is consumed instead of closing it again.
            if (hoverOpenedRef.current) { hoverOpenedRef.current = false; return; }
            setPickerOpen((o) => !o);
          }}>
          <SmileyIcon size={14} />
        </button>
      )}
      {!m.deleted && (canEdit || canDelete) && (
        <button type="button" className="msg-bubble-action msg-bubble-action--menu"
          aria-label="Message options" aria-expanded={menuOpen}
          onClick={(e) => { e.stopPropagation(); setMenuOpen((o) => !o); }}>
          <ChevronDownIcon size={14} />
        </button>
      )}
      {m.deleted ? (
        <em style={{ opacity: 0.5, fontStyle: "italic" }}>Message deleted</em>
      ) : m.decrypt_failed ? (
        /* Checked before `editing`: the content is empty, so edit mode would
           otherwise offer to edit a blank string and overwrite the only
           remaining copy of the plaintext with nothing. */
        <em style={{ opacity: 0.7, fontStyle: "italic" }}>
          Unable to decrypt this message
        </em>
      ) : editing ? (
        /* Inline edit mode */
        <div className="msg-edit-inline">
          <textarea className="msg-edit-input" value={editText}
            onChange={(e) => setEditText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                void saveEdit();
              }
              if (e.key === "Escape") { e.stopPropagation(); setEditing(false); }
            }}
            autoFocus
            rows={2}
          />
          <div className="msg-edit-actions">
            <Button size="xs" onClick={() => { void saveEdit(); }}>Save</Button>
            <Button variant="secondary" size="xs" onClick={() => setEditing(false)}>Cancel</Button>
          </div>
        </div>
      ) : (
        /* Normal message rendering */
        <div>
          {/* Sender label for group messages */}
          {m.sender_peer_key_hex && (m.sender_peer_key_hex.length > 0) && (
            <div className="msg-sender-label">
              {m.sender_peer_key_hex.substring(0, 8)}…
            </div>
          )}
          <div className="msg-content">{plain ? m.content : renderMarkdown(m.content)}</div>
        </div>
      )}
      <span className="msg-footer-row">
        <span className="msg-time">{new Date(m.timestamp * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
        {/* Message status for sent messages */}
        {m.direction === "sent" && !m.deleted && msgStatus && (
          <span className={`msg-status msg-status--${msgStatus}`}>
            {msgStatus === "sending" && <ClockIcon size={10} />}
            {msgStatus === "sent" && "✓"}
            {msgStatus === "delivered" && <CheckDoubleIcon size={12} />}
            {msgStatus === "read" && <CheckDoubleIcon size={12} />}
          </span>
        )}
        {/* Edited badge */}
        {m.edited_at !== null && !m.deleted && (
          <span className="msg-edited-badge" title={`Edited ${new Date(m.edited_at * 1000).toLocaleString()}`}>edited</span>
        )}
        {/* Self-destruct timer */}
        {m.expires_at !== null && !m.deleted && !m.direction.startsWith("sent") && (
          <SelfDestructTimer expiresAt={m.expires_at} />
        )}
        {/* Read receipt for received messages */}
        {m.direction === "received" && m.read_at !== null && !m.deleted && (
          <span className="msg-read-badge" title={`Read ${new Date(m.read_at * 1000).toLocaleString()}`}>
            ✓✓
          </span>
        )}
      </span>
      {/* Reactions */}
      {Object.keys(m.reactions || {}).length > 0 && !m.deleted && (
        <div className="msg-reactions">
          {Object.entries(m.reactions).map(([emoji, reactors]) => (
            <button
              key={emoji}
              className={`msg-reaction ${reactedByMe(reactors) ? "msg-reaction--self" : ""}`}
              aria-label={"React " + emoji}
              aria-pressed={reactedByMe(reactors)}
              onClick={() => {
                if (reactedByMe(reactors)) {
                  onRemoveReaction?.(m.id, emoji);
                } else {
                  onReact?.(m.id, emoji);
                }
              }}
              title={reactors.join(", ")}
            >
              {emoji} {reactors.length}
            </button>
          ))}
        </div>
      )}
      {/* Reaction picker */}
      {pickerOpen && !m.deleted && canReact && (
        <div className="reaction-picker">
          {PICKER_EMOJIS.map((emoji) => (
            <button
              key={emoji}
              className={`reaction-picker__btn ${reactedByMe(m.reactions?.[emoji] || []) ? "reaction-picker__btn--active" : ""}`}
              aria-label={"React " + emoji}
              aria-pressed={reactedByMe(m.reactions?.[emoji] || [])}
              onClick={(e) => {
                e.stopPropagation();
                const reactors = m.reactions?.[emoji] || [];
                if (reactedByMe(reactors)) {
                  onRemoveReaction?.(m.id, emoji);
                } else {
                  onReact?.(m.id, emoji);
                }
              }}
            >
              {emoji}
            </button>
          ))}
        </div>
      )}
      {/* Context menu */}
      {menuOpen && !m.deleted && (canEdit || canDelete) && (
        <div className="msg-context-menu" onClick={(e) => e.stopPropagation()}>
          {canEdit && (
            <button className="msg-context-item" onClick={() => { setEditText(m.content); setEditing(true); setMenuOpen(false); }}>
              Edit
            </button>
          )}
          {canDelete && (
            <button className="msg-context-item msg-context-item--danger" onClick={async () => { setMenuOpen(false); await onDelete?.(m.id); }}>
              Delete
            </button>
          )}
        </div>
      )}
    </div>
  );
}
