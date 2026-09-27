/**
 * Tauri event payload types and runtime validators.
 *
 * ## Why this file exists
 *
 * Every `listen()` call in this app was typed `listen<any>`. That is not a
 * cosmetic typing complaint: **the payloads carried here contain
 * peer-controlled data.** A message body, a filename, a group name, an emoji,
 * a reaction key — all of it arrives from another person over the network and
 * lands directly in React state, and from there into `renderMarkdown` (the one
 * place remote text becomes DOM) and into attributes like `className`.
 *
 * A TypeScript type is erased at runtime, so `listen<any>` was providing
 * literally zero checking. These validators turn that back into a real
 * boundary: a malformed or hostile payload is dropped with a warning instead
 * of being trusted.
 *
 * ## Shape
 *
 * Each validator is a type guard: it returns the payload typed, or `null`.
 * The pattern is always:
 *
 * ```ts
 * const payload = asConnectionEvent(event.payload);
 * if (!payload) return;   // malformed — ignore, never partially use
 * ```
 *
 * ## Why "drop, don't repair"
 *
 * A validator that filled in missing fields would let a peer omit `state` and
 * get whatever default we chose — and `state` drives view navigation. Failing
 * closed means a malformed payload is simply not acted on, which is the only
 * safe default for untrusted input.
 */

import type { ChatMessage } from "./types";

// ─── Primitive guards ───────────────────────────────────────────────────────

function isString(v: unknown): v is string {
  return typeof v === "string";
}

function isBool(v: unknown): v is boolean {
  return typeof v === "boolean";
}

/** Finite non-negative number, or null when optional. */
function isU64(v: unknown): v is number {
  return typeof v === "number" && Number.isFinite(v) && v >= 0;
}

function isU32(v: unknown): v is number {
  return isU64(v) && Number.isInteger(v) && v <= 0xFFFF_FFFF;
}

function isStringOrNull(v: unknown): v is string | null {
  return v === null || v === undefined || isString(v);
}

/**
 * A peer key: 64 lowercase-or-uppercase hex characters (32 bytes).
 *
 * `peer_key_hex` is used as a SQLite lookup key, as a notification group, and
 * as a React list key. Validating the shape here means a malformed value can
 * never reach any of those.
 */
function isPeerKeyHex(v: unknown): v is string {
  return isString(v) && /^[0-9a-fA-F]{64}$/.test(v);
}

/** A transfer id: bounded printable ASCII, no control characters. */
function isOpaqueId(v: unknown): v is string {
  return isString(v) && v.length > 0 && v.length <= 128 && /^[\x20-\x7e]+$/.test(v);
}

/**
 * A peer-supplied display string: filename, group name, display name.
 *
 * Length-bounded and stripped of control characters. This is not about XSS —
 * React escapes on render — it is about not letting a peer push a 10 MB
 * "filename" into the DOM, or a string containing newlines that reflows and
 * misleads the layout.
 */
const MAX_LABEL_LEN = 512;
function isDisplayText(v: unknown): v is string {
  return (
    isString(v) &&
    v.length <= MAX_LABEL_LEN &&
    !/[\u0000-\u001f\u007f]/.test(v)
  );
}

/** A reaction emoji: short, and not used as an unbounded object key. */
function isReactionEmoji(v: unknown): v is string {
  return isString(v) && v.length > 0 && v.length <= 16 && Array.from(v).length <= 4;
}

// ─── ChatMessage ────────────────────────────────────────────────────────────

/**
 * Validate a `ChatMessage` from the wire.
 *
 * `direction` is constrained to the two values the app actually produces. It
 * used to be typed `string`, and the test fixtures used `"incoming"` /
 * `"outgoing"` — values the app never emits — so the message-rendering tests
 * were exercising a branch that could never run in production.
 */
export function asChatMessage(v: unknown): ChatMessage | null {
  if (typeof v !== "object" || v === null) return null;
  const m = v as Record<string, unknown>;

  if (!isString(m.id) || m.id.length > 128) return null;
  if (!isString(m.content) || m.content.length > 64 * 1024) return null;
  if (m.direction !== "sent" && m.direction !== "received") return null;
  if (!isU64(m.timestamp)) return null;
  if (!isStringOrNull(m.read_at) || (m.read_at !== null && !isU64(m.read_at))) return null;
  if (!isStringOrNull(m.edited_at) || (m.edited_at !== null && !isU64(m.edited_at))) return null;
  if (m.deleted !== undefined && !isBool(m.deleted)) return null;
  if (!isStringOrNull(m.expires_at) || (m.expires_at !== null && !isU64(m.expires_at))) return null;
  // A 1:1 message has an EMPTY sender key by design: the peer is implicit from
  // the conversation, and the Rust side documents this explicitly
  // ("Empty string for 1:1 messages (implicit from conversation)" on
  // ChatMessage.sender_peer_key_hex, defaulting to String::new() in
  // ChatMessage::new). Only group messages carry a real key.
  //
  // Requiring a 64-char key here rejected every direct message, so the chat
  // listener dropped 100% of 1:1 traffic. The test fixtures all supplied a
  // group-style key, which is exactly why the suite stayed green.
  if (!isString(m.sender_peer_key_hex)) return null;
  if (m.sender_peer_key_hex !== "" && !isPeerKeyHex(m.sender_peer_key_hex)) return null;

  // `reactions` becomes object keys AND visible labels, so both the key and
  // the values are bounded.
  if (m.reactions !== undefined) {
    if (typeof m.reactions !== "object" || m.reactions === null || Array.isArray(m.reactions)) {
      return null;
    }
    const entries = Object.entries(m.reactions as Record<string, unknown>);
    if (entries.length > 32) return null;
    for (const [emoji, peers] of entries) {
      if (!isReactionEmoji(emoji)) return null;
      if (!Array.isArray(peers) || peers.length > 64) return null;
      if (!peers.every(isPeerKeyHex)) return null;
    }
  }

  return {
    id: m.id,
    content: m.content,
    direction: m.direction,
    timestamp: m.timestamp,
    read_at: (m.read_at as number | null | undefined) ?? null,
    edited_at: (m.edited_at as number | null | undefined) ?? null,
    deleted: m.deleted ?? false,
    expires_at: (m.expires_at as number | null | undefined) ?? null,
    reactions: (m.reactions as Record<string, string[]> | undefined) ?? {},
    sender_peer_key_hex: m.sender_peer_key_hex,
  } as ChatMessage;
}

// ─── Events ─────────────────────────────────────────────────────────────────

/** `m2m://message` — an inbound chat message. */
export interface MessageEventPayload {
  peer_key_hex: string;
  message: ChatMessage;
}

export function asMessageEvent(v: unknown): MessageEventPayload | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isPeerKeyHex(p.peer_key_hex)) return null;
  const message = asChatMessage(p.message);
  if (!message) return null;
  return { peer_key_hex: p.peer_key_hex, message };
}

/** Known connection states the backend emits. */
export const CONNECTION_STATES = ["established", "disconnected"] as const;
export type ConnectionState = (typeof CONNECTION_STATES)[number];

export interface ConnectionEventPayload {
  peer_key_hex: string;
  state: ConnectionState;
  peer_fingerprint: string | null;
  peer_verified: boolean;
}

export function asConnectionEvent(v: unknown): ConnectionEventPayload | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isPeerKeyHex(p.peer_key_hex)) return null;
  if (p.state !== "established" && p.state !== "disconnected") return null;
  if (!isStringOrNull(p.peer_fingerprint)) return null;
  if (p.peer_verified !== undefined && !isBool(p.peer_verified)) return null;
  return {
    peer_key_hex: p.peer_key_hex,
    state: p.state,
    peer_fingerprint: (p.peer_fingerprint as string | null | undefined) ?? null,
    peer_verified: p.peer_verified ?? false,
  };
}

export interface FileRequestEventPayload {
  peer_key_hex: string;
  transfer_id: string;
  filename: string;
  total_size: number;
}

export function asFileRequestEvent(v: unknown): FileRequestEventPayload | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isPeerKeyHex(p.peer_key_hex)) return null;
  if (!isOpaqueId(p.transfer_id)) return null;
  if (!isDisplayText(p.filename)) return null;
  if (!isU64(p.total_size)) return null;
  return {
    peer_key_hex: p.peer_key_hex,
    transfer_id: p.transfer_id,
    filename: p.filename,
    total_size: p.total_size,
  };
}

export interface TransferProgressEventPayload {
  transfer_id: string;
  peer_key_hex: string;
  filename: string;
  total_size: number;
  bytes_transferred: number;
  chunks_completed: number;
  chunks_total: number;
  state: string;
  speed_bytes_per_sec: number;
  estimated_remaining_secs: number;
}

/** Transfer states the backend reports. Bounded to a known set for the same
 *  reason connection states are: `state` is interpolated into a className. */
const TRANSFER_STATES = new Set([
  "requested",
  "accepted",
  "rejected",
  "sending",
  "receiving",
  "verifying",
  "completed",
  "cancelled",
  "failed",
]);

export function asTransferProgressEvent(v: unknown): TransferProgressEventPayload | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isOpaqueId(p.transfer_id)) return null;
  if (!isPeerKeyHex(p.peer_key_hex)) return null;
  if (!isDisplayText(p.filename)) return null;
  if (!isU64(p.total_size) || !isU64(p.bytes_transferred)) return null;
  if (!isU32(p.chunks_completed) || !isU32(p.chunks_total)) return null;
  if (!isString(p.state) || !TRANSFER_STATES.has(p.state)) return null;
  if (!isU64(p.speed_bytes_per_sec) || !isU64(p.estimated_remaining_secs)) return null;
  return {
    transfer_id: p.transfer_id,
    peer_key_hex: p.peer_key_hex,
    filename: p.filename,
    total_size: p.total_size,
    bytes_transferred: p.bytes_transferred,
    chunks_completed: p.chunks_completed,
    chunks_total: p.chunks_total,
    state: p.state,
    speed_bytes_per_sec: p.speed_bytes_per_sec,
    estimated_remaining_secs: p.estimated_remaining_secs,
  };
}

/** `m2m://transfer-completed` — note the backend sends ONLY `transfer_id`. */
export function asTransferCompletedEvent(v: unknown): { transfer_id: string } | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isOpaqueId(p.transfer_id)) return null;
  return { transfer_id: p.transfer_id };
}

export function asTransferErrorEvent(
  v: unknown,
): { transfer_id: string; error: string } | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isOpaqueId(p.transfer_id)) return null;
  if (!isDisplayText(p.error)) return null;
  return { transfer_id: p.transfer_id, error: p.error };
}

export function asTransferCancelledEvent(v: unknown): { transfer_id: string } | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isOpaqueId(p.transfer_id)) return null;
  return { transfer_id: p.transfer_id };
}

export interface ReactionEventPayload {
  message_id: string;
  reaction: string;
  peer_key_hex: string;
  remove: boolean;
}

export function asReactionEvent(v: unknown): ReactionEventPayload | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isString(p.message_id) || p.message_id.length > 128) return null;
  if (!isReactionEmoji(p.reaction)) return null;
  if (!isPeerKeyHex(p.peer_key_hex)) return null;
  if (typeof p.remove !== "boolean") return null;
  return {
    message_id: p.message_id,
    reaction: p.reaction,
    peer_key_hex: p.peer_key_hex,
    remove: p.remove,
  };
}

export interface EditEventPayload {
  message_id: string;
  new_content: string;
  edited_at: number;
  peer_key_hex: string;
}

export function asEditEvent(v: unknown): EditEventPayload | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isString(p.message_id) || p.message_id.length > 128) return null;
  // `new_content` replaces the rendered body and goes through renderMarkdown.
  if (!isString(p.new_content) || p.new_content.length > 64 * 1024) return null;
  if (!isU64(p.edited_at)) return null;
  if (!isPeerKeyHex(p.peer_key_hex)) return null;
  return {
    message_id: p.message_id,
    new_content: p.new_content,
    edited_at: p.edited_at,
    peer_key_hex: p.peer_key_hex,
  };
}

export function asDeleteEvent(v: unknown): { message_id: string; peer_key_hex: string } | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isString(p.message_id) || p.message_id.length > 128) return null;
  if (!isPeerKeyHex(p.peer_key_hex)) return null;
  return { message_id: p.message_id, peer_key_hex: p.peer_key_hex };
}

export function asTypingEvent(v: unknown): { peer_key_hex: string; typing: boolean } | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isPeerKeyHex(p.peer_key_hex)) return null;
  if (typeof p.typing !== "boolean") return null;
  return { peer_key_hex: p.peer_key_hex, typing: p.typing };
}

/**
 * Reconnect attempt states.
 *
 * The backend also emits `"handshake_failed"`, which the old untyped handler
 * did not handle — leaving `reconnecting` stuck `true` and the UI showing
 * "Reconnecting (N/5)…" indefinitely. It is now a known state.
 */
export const RECONNECT_STATES = [
  "attempting",
  "success",
  "failed",
  "handshake_failed",
] as const;
export type ReconnectState = (typeof RECONNECT_STATES)[number];

export interface ReconnectAttemptPayload {
  peer_key_hex: string;
  attempt: number;
  max_attempts: number;
  delay_secs: number;
  state: ReconnectState;
}

export function asReconnectAttempt(v: unknown): ReconnectAttemptPayload | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isPeerKeyHex(p.peer_key_hex)) return null;
  if (!isU32(p.attempt) || !isU32(p.max_attempts) || !isU64(p.delay_secs)) return null;
  if (!RECONNECT_STATES.includes(p.state as ReconnectState)) return null;
  return {
    peer_key_hex: p.peer_key_hex,
    attempt: p.attempt,
    max_attempts: p.max_attempts,
    delay_secs: p.delay_secs,
    state: p.state as ReconnectState,
  };
}

const GROUP_EVENT_TYPES = new Set([
  "created",
  "invited",
  "member_added",
  "member_removed",
  "member_left",
  "name_changed",
]);

export interface GroupEventPayload {
  group_id: string;
  event_type: string;
  peer_key_hex: string | null;
}

export function asGroupEvent(v: unknown): GroupEventPayload | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isString(p.group_id) || p.group_id.length > 128) return null;
  if (!isString(p.event_type) || !GROUP_EVENT_TYPES.has(p.event_type)) return null;
  if (!isStringOrNull(p.peer_key_hex)) return null;
  if (p.peer_key_hex !== null && p.peer_key_hex !== undefined && !isPeerKeyHex(p.peer_key_hex)) {
    return null;
  }
  return {
    group_id: p.group_id,
    event_type: p.event_type,
    peer_key_hex: (p.peer_key_hex as string | null | undefined) ?? null,
  };
}

export function asGroupMessageEvent(
  v: unknown,
): { group_id: string; message: ChatMessage } | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isString(p.group_id) || p.group_id.length > 128) return null;
  const message = asChatMessage(p.message);
  if (!message) return null;
  return { group_id: p.group_id, message };
}

export function asCaptureWarning(v: unknown): { active: string[] } | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!Array.isArray(p.active) || p.active.length > 32) return null;
  if (!p.active.every(isDisplayText)) return null;
  return { active: p.active };
}

export function asVaultLocked(v: unknown): Record<string, never> | null {
  // The event carries no payload; only its arrival matters.
  if (typeof v !== "object" || v === null) return null;
  return {};
}

export function asSyncStatus(v: unknown): { status: string; peer_key_hex: string } | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isString(p.status) || p.status.length > 32) return null;
  if (!isPeerKeyHex(p.peer_key_hex)) return null;
  return { status: p.status, peer_key_hex: p.peer_key_hex };
}

export function asSyncDevice(
  v: unknown,
): { device_id: string; device_name: string } | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isString(p.device_id) || p.device_id.length > 128) return null;
  // `device_name` is peer-supplied and rendered in the UI.
  if (!isDisplayText(p.device_name)) return null;
  return { device_id: p.device_id, device_name: p.device_name };
}

/**
 * Emitted when a security control FAILS to apply. Surfaced so a silently
 * missing protection is not silent.
 */
export function asSecurityError(v: unknown): { source: string; message: string } | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isString(p.source) || p.source.length > 64) return null;
  if (!isDisplayText(p.message)) return null;
  return { source: p.source, message: p.message };
}

/** `m2m://conversation-meta` — a peer-supplied suggested display name. */
export interface ConversationMetaPayload {
  peer_key_hex: string;
  peer_display_name: string;
  suggested_name: string;
}

export function asConversationMeta(v: unknown): ConversationMetaPayload | null {
  if (typeof v !== "object" || v === null) return null;
  const p = v as Record<string, unknown>;
  if (!isPeerKeyHex(p.peer_key_hex)) return null;
  // Both names are peer-supplied free text that the backend writes into the
  // conversation table and the UI renders.
  if (!isDisplayText(p.peer_display_name)) return null;
  if (!isDisplayText(p.suggested_name)) return null;
  return {
    peer_key_hex: p.peer_key_hex,
    peer_display_name: p.peer_display_name,
    suggested_name: p.suggested_name,
  };
}
