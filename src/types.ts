// ─── Shared Types for M2M Frontend ───

export interface Toast {
  id: string;
  message: string;
  type: "success" | "error" | "info" | "warning";
  duration?: number;
}

export interface ConversationEntry {
  id: string;
  peer_key_hex: string;
  display_name: string | null;
  peer_display_name: string | null;
  last_message_at: number | null;
  last_message_preview: string | null;
  message_count: number;
  is_online: boolean;
  auto_delete_at: number | null;
  retention_policy: string;
  created_at: number;
  is_favorite?: boolean;
  archived?: boolean;
  unread_count?: number;
}

export interface IdentityInfo {
  fingerprint: string;
  public_key_hex: string;
  has_identity: boolean;
}

export interface ChatMessage {
  id: string;
  content: string;
  /**
   * Narrowed from the validator.
   *
   * This was `string`, while `events.ts` correctly restricted it to exactly
   * `"sent" | "received"`. The mismatch matters: `MessageBubble` interpolates
   * this straight into a className (`msg-bubble--${m.direction}`), so the loose
   * type permitted the class injection that `events.ts` exists to prevent — and
   * it was reachable on the non-event path too, where `handleSendFile`
   * synthesises a message with a cast.
   */
  direction: "sent" | "received";
  timestamp: number;
  /// When this message was read (null = unread, only for received messages).
  read_at: number | null;
  /// When this message was edited (null = never).
  edited_at: number | null;
  /// Whether this message has been soft-deleted.
  deleted: boolean;
  /// When this message self-destructs (null = never, 0 = already expired).
  expires_at: number | null;
  /// Reactions on this message, as a map: reaction_emoji → [peer_key_hex, ...].
  reactions: Record<string, string[]>;
  /// Sender of this message (used for group messages).
  sender_peer_key_hex: string;
}

export interface ConnectionInfo {
  state: string;
  peer_fingerprint: string | null;
  peer_verified: boolean;
  peer_key_hex: string | null;
}

export interface FileRequest {
  peer_key_hex: string;
  transfer_id: string;
  filename: string;
  total_size: number;
}

export interface VaultStatus {
  initialized: boolean;
  unlocked: boolean;
}

export interface NetworkSettings {
  tor_enabled: boolean;
  tor_proxy_addr: string;
  tor_reachable: boolean;
  public_ip: string | null;
}

export interface StunConfig {
  servers: string[];
  timeout_secs: number;
  private_mode: boolean;
}

export interface DiscoveryConfig {
  lan_enabled: boolean;
  dht_enabled: boolean;
}

export interface DiscoveredPeer {
  id_hex: string;
  address: string;
  method: "lan" | "dht";
  last_seen: number;
}

export interface FamilyMember {
  public_key_hex: string;
  nickname: string;
  added_at: number;
  expires_at: number | null;
  last_address: string | null;
}

export interface GroupInfo {
  group_id: string;
  group_name: string;
  member_count: number;
  created_at: number;
}

export interface GroupMember {
  peer_key_hex: string;
  display_name: string | null;
  role: "admin" | "member";
  added_at: number;
}

export interface GroupDetail {
  group_id: string;
  group_name: string;
  member_count: number;
  created_at: number;
  our_role: string;
  members: GroupMember[];
}

export interface SecurityConfig {
  screen_capture_protection: boolean;
  clipboard_clear_secs: number;
  idle_lock_secs: number;
  require_known_contact: boolean;
  capture_process_detection: boolean;
  blur_on_focus_loss: boolean;
  air_gap_mode: boolean;
  ephemeral_mode: boolean;
  send_batching_ms: number;
  cover_typing_traffic: boolean;
  panic_hotkey_enabled: boolean;
  /**
   * Maximum stored message history in bytes. `0` means "use the default"
   * (10 GiB), NOT "unlimited" — the backend resolves it via
   * `effective_storage_cap()`, and `SecurityConfig::default()` produces 0.
   *
   * When usage exceeds the cap, the oldest messages are permanently evicted.
   */
  storage_cap_bytes: number;
}

/** Storage usage reported by `get_storage_usage`. */
export interface StorageUsage {
  used_bytes: number;
  cap_bytes: number;
}

/** Honest per-platform capability report for screen-capture protection. */
export interface CaptureCapability {
  level: "full" | "partial" | "unsupported";
  note: string;
}

export interface TransferProgress {
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

/**
 * Health of one configured STUN server.
 *
 * Mirrors `stun::StunServerHealth`. `rtt_ms` is `null` when the probe did not
 * complete, which is why it is nullable rather than `0`.
 */
export interface StunServerHealth {
  server: string;
  reachable: boolean;
  rtt_ms: number | null;
  error: string | null;
}

/**
 * Result of `check_connectivity`.
 *
 * Mirrors `stun::ConnectivityStatus`. `public_addr` and `host_addrs` are the
 * user's own addresses, so anything rendering this must treat it as sensitive.
 */
export interface ConnectivityStatus {
  /**
   * Whether the listening port is reachable from the public internet.
   *
   * `null` means **not measured**, which is the honest answer from every code
   * path the backend has. This was a plain `boolean` derived from
   * `NatType::Symmetric` or from whether the STUN servers agreed — neither of
   * which tests inbound reachability of a *TCP* port; the STUN-mapped UDP port
   * belongs to a throwaway probe socket and is generally not the listening port
   * at all. A symmetric-NAT user, the case that most needs TURN, was being told
   * "reachable: true". Measuring it for real needs a third party to dial our
   * port, which discloses the address, so it is reported as unmeasured rather
   * than assumed. Render it as "not measured", never as "no".
   */
  reachable: boolean | null;
  /**
   * Whether the STUN servers agreed on this node's public address — the one
   * fact a local connectivity check *can* establish. `null` when not checked.
   */
  stun_agreement: boolean | null;
  nat_type: string;
  public_addr: string | null;
  host_addrs: string[];
  behind_symmetric_nat: boolean;
}

/** Result of `validate_invite`. Mirrors `commands::InviteInfo`. */
export interface InviteInfo {
  fingerprint: string;
  address_hint: string;
  expires_at: number;
  one_time: boolean;
  valid: boolean;
}

/** Persisted theme preference. Mirrors `commands::ThemePreference`. */
export interface ThemePreference {
  theme: string;
  accent_color: string;
}

export interface NatTypeInfo {
  nat_type: string;
  stun_servers: Array<{
    server: string;
    reachable: boolean;
    rtt_ms: number | null;
    error: string | null;
  }>;
  connectivity: {
    /** See `ConnectivityStatus.reachable` — `null` means not measured. */
    reachable: boolean | null;
    /** See `ConnectivityStatus.stun_agreement` — `null` means not checked. */
    stun_agreement: boolean | null;
    nat_type: string;
    public_addr: string | null;
    host_addrs: string[];
    behind_symmetric_nat: boolean;
  };
  candidates: Array<{
    address: string;
    candidate_type: number;
    priority: number;
  }>;
}
