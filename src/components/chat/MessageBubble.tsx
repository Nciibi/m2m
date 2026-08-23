import { useState, useEffect } from "react";
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
}: MessageBubbleProps) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [editing, setEditing] = useState(false);
  const [editText, setEditText] = useState("");

  // Close context menu on click outside
  useEffect(() => {
    if (!menuOpen) return;
    const handler = () => setMenuOpen(false);
    window.addEventListener("click", handler, { once: true });
    return () => window.removeEventListener("click", handler);
  }, [menuOpen]);

  const canEdit = typeof onEditSave === "function";
  const canDelete = typeof onDelete === "function";
  const canReact = typeof onReact === "function" || typeof onRemoveReaction === "function";
  const senderLabel = m.direction === "sent" ? "you" : m.sender_peer_key_hex ? m.sender_peer_key_hex.substring(0, 8) : "peer";

  return (
    <div
      className={`msg-bubble msg-bubble--${m.direction}${m.deleted ? " msg-bubble--deleted" : ""}`}
      style={{ animationDelay: `${index * 0.05}s` }}
      tabIndex={0}
      role="group"
      aria-label={`Message from ${senderLabel}: ${m.content.substring(0, 40)}`}
      onMouseEnter={() => canReact && setPickerOpen(true)}
      onMouseLeave={() => setPickerOpen(false)}
      onContextMenu={(e) => { if (!canEdit && !canDelete) return; e.preventDefault(); setMenuOpen(true); }}
      onKeyDown={(e) => { if (e.key === "Escape") { if (pickerOpen) { setPickerOpen(false); e.stopPropagation(); } if (menuOpen) { setMenuOpen(false); e.stopPropagation(); } } }}
    >
      {!m.deleted && canReact && (
        <button type="button" className="msg-bubble-action msg-bubble-action--react"
          aria-label="Toggle reaction picker" aria-expanded={pickerOpen}
          onClick={(e) => { e.stopPropagation(); setPickerOpen((o) => !o); }}>
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
      ) : editing ? (
        /* Inline edit mode */
        <div className="msg-edit-inline">
          <textarea className="msg-edit-input" value={editText}
            onChange={(e) => setEditText(e.target.value)}
            onKeyDown={async (e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                await onEditSave?.(m.id, editText);
                setEditing(false);
              }
              if (e.key === "Escape") { e.stopPropagation(); setEditing(false); }
            }}
            autoFocus
            rows={2}
          />
          <div className="msg-edit-actions">
            <Button size="xs" onClick={async () => { await onEditSave?.(m.id, editText); setEditing(false); }}>Save</Button>
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
        {m.direction === "received" && m.read_at !== null && (
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
              className={`msg-reaction ${reactors.includes("self") ? "msg-reaction--self" : ""}`}
              aria-label={"React " + emoji}
              aria-pressed={reactors.includes("self")}
              onClick={() => {
                if (reactors.includes("self")) {
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
              className={`reaction-picker__btn ${(m.reactions?.[emoji] || []).includes("self") ? "reaction-picker__btn--active" : ""}`}
              aria-label={"React " + emoji}
              aria-pressed={(m.reactions?.[emoji] || []).includes("self")}
              onClick={(e) => {
                e.stopPropagation();
                const reactors = m.reactions?.[emoji] || [];
                if (reactors.includes("self")) {
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
