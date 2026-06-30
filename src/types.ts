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
}

export interface IdentityInfo {
  fingerprint: string;
  public_key_hex: string;
  has_identity: boolean;
}

export interface ChatMessage {
  id: string;
  content: string;
  direction: string;
  timestamp: number;
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

export interface NatTypeInfo {
  nat_type: string;
  stun_servers: Array<{
    server: string;
    reachable: boolean;
    rtt_ms: number | null;
    error: string | null;
  }>;
  connectivity: {
    reachable: boolean;
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
