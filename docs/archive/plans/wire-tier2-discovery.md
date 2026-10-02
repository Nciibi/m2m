# Wire Tier 2 Discovery

## Goal
Wire DHT and LAN peer discovery into `AppState`, Tauri commands, and the frontend settings/hub UI — both as **optional, OFF-by-default** discovery methods alongside invite-based connections.

## Design Principles
1. **Privacy-first**: Default OFF, ephemeral IDs only, no permanent key exposure
2. **Follow existing patterns**: Mirror Tor toggles and relay commands precisely
3. **Invite-first identity**: Discovered peers need out-of-band fingerprint verification before messaging
4. **Minimal surface**: No new crypto, no new protocol packets

## Files to Touch

### Backend (Rust)

| File | Change |
|---|---|
| `src-tauri/src/state.rs` | Add `DiscoveryConfig` struct + `discovery_config` field to `AppState` |
| `src-tauri/src/commands/discovery.rs` | **New file** — 5 Tauri commands |
| `src-tauri/src/commands/mod.rs` | Add `pub mod discovery` |
| `src-tauri/src/lib.rs` | Register 5 new commands in `invoke_handler` |
| `src-tauri/src/dht.rs` | Add `DhtState::enabled()` helper; make `announce_loop` stoppable via a `CancellationToken` |
| `src-tauri/src/lan_discovery.rs` | Add `LanDiscoveryState::enabled()` helper; make `start` return a cancel handle |
| `src-tauri/src/ephemeral_id.rs` | Already clean — no changes needed |

### Frontend (TypeScript/React)

| File | Change |
|---|---|
| `src/types.ts` | Add `DiscoveryConfig`, `DiscoveredPeer` types |
| `src/context/SettingsContext.tsx` | Add discovery state + toggle handlers |
| `src/views/SettingsView.tsx` | Add LAN / DHT toggle section |
| `src/views/HubView.tsx` | Add "Nearby" section for discovered peers |
| `src/__tests__/SettingsContext.test.tsx` | Add discovery toggle tests |

## Step-by-step

### 1. Add DiscoveryConfig to AppState

```rust
// state.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// LAN multicast discovery — OFF by default.
    pub lan_enabled: bool,
    /// DHT peer discovery — OFF by default.
    pub dht_enabled: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            lan_enabled: false,  // ⚠️ OFF by default
            dht_enabled: false,  // ⚠️ OFF by default
        }
    }
}

// In AppState:
pub discovery_config: RwLock<DiscoveryConfig>,
pub dht_state: RwLock<Option<Arc<dht::DhtState>>>,
pub lan_state: RwLock<Option<Arc<lan_discovery::LanDiscoveryState>>>,
```

This adds the runtime state for both discovery modules, mirroring how `relay_config` / `relay_state` are handled.

### 2. New commands/discovery.rs (5 commands)

- **`get_discovery_config`** — returns current DiscoveryConfig
- **`set_discovery_config`** — updates config, starts/stops services:
  - Enabling LAN: spawns `lan_discovery::start()` background tasks, stores cancel handle
  - Disabling LAN: signals cancellation, cleans up
  - Enabling DHT: spawns `dht::announce_loop()`, stores cancel handle
  - Disabling DHT: signals cancellation, cleans up
- **`get_discovered_peers`** — returns merged list of LAN peers + DHT peers
- **`connect_discovered_peer`** — connects to a discovered peer without an invite:
  1. Do the standard handshake (without `expected_peer_pub` pre-check)
  2. After handshake, check key store for this peer's identity
  3. If known → auto-trust, start receive loop
  4. If unknown → session established as "unverified", require fingerprint verification
- **`refresh_discovery`** — force-refresh discovery state (e.g., after network change)

### 3. DHT/LAN lifecycle management

Both `dht::announce_loop` and `lan_discovery::start` currently run forever (`loop { ... }`). Add `CancellationToken` support:

- `dht::announce_loop(dht_state, ephemeral_id, network_monitor, listen_addr, cancel: CancellationToken)`
- `lan_discovery::start(listen_addr, lan_state, ephemeral_id, cancel: CancellationToken)`

The `CancellationToken` approach is clean and doesn't require `Arc<AtomicBool>` polling.

### 4. Discovered peer connection flow

New `connect_discovered_peer` command in `discovery.rs`:

1. Receive `peer_addr: SocketAddr` and `discovery_method: "lan" | "dht"`
2. Use `tor::connect(peer_addr)` (respecting Tor setting) to open TCP
3. Create a `Session`, perform standard handshake with `[0u8; 32]` as expected_peer_pub (skip identity pre-check)
4. After handshake, extract actual `peer_identity_pub` from session
5. Look up in key store — if exists, auto-trust; if not, require verification
6. Start receive loop, emit connection event
7. The UI shows "unverified" badge until user verifies fingerprint

### 5. Frontend — SettingsContext

Add to `SettingsContextValue`:
```typescript
discoveryConfig: DiscoveryConfig | null;
lanEnabled: boolean;
dhtEnabled: boolean;
handleLanToggle: () => Promise<void>;
handleDhtToggle: () => Promise<void>;
handleConnectDiscoveredPeer: (peerAddr: string, method: "lan" | "dht") => Promise<void>;
```

Handlers call `get_discovery_config` / `set_discovery_config` on the backend, mirroring the Tor toggle pattern exactly.

### 6. Frontend — SettingsView

Add a "Discovery" section below Network:
```
┌─ Discovery ─────────────────────────────────┐
│ LAN Discovery  [OFF/ON toggle]              │
│ DHT Discovery  [OFF/ON toggle]              │
│                                              │
│ ⚠️ OFF by default — see privacy warning      │
└──────────────────────────────────────────────┘
```

Privacy warning tooltip/modal on first enable:
- LAN: "Your presence is broadcast on the local WiFi. Only use on trusted networks."
- DHT: "Your IP is visible to DHT nodes. Ephemeral ID rotates every 24h."

### 7. Frontend — HubView

Add a "Nearby" section below conversations:
```
┌─ Nearby ────────────────────────────────────┐
│ 🔵 Desktop (10.0.0.5)                       │
│    Fingerprint:  a1b2...  [Connect]         │
│                                              │
│ 🔵 Unknown device (10.0.0.12)               │
│    Fingerprint:  e3f4...  [Connect] [Invite] │
│                                              │
│ Discovery not active — enable in Settings    │
└──────────────────────────────────────────────┘
```

- Known peers → "Connect" button → `connect_discovered_peer`
- Unknown peers → "Connect" + "Invite" (generate invite link to share)
- Empty state when discovery is off → link to settings

## What we DON'T change

- ❌ No new protocol packets
- ❌ No new crypto primitives
- ❌ No changes to session handshake internals (just the pre-check skip)
- ❌ No persistence of discovery config (it's a runtime toggle, like Tor)
- ❌ No DHT bootstrap node configuration in this pass (future work)
- ❌ No DHT maintenance CLI (future work)

## Edge Cases & Privacy

- **Privacy mode + Discovery**: If private mode is ON but user enables DHT, warn that DHT bypasses private mode (your IP is visible to DHT nodes regardless). Offer to disable DHT.
- **Both ON**: LAN and DHT can run simultaneously.
- **Tor + Discovery**: If Tor is ON and user enables DHT, warn that DHT traffic doesn't go through Tor. DHT announce bypasses Tor.
- **Network change**: Both DHT and LAN already rotate ephemeral IDs on network change via `NetworkMonitor`.
- **Concurrent lifecycle**: Starting discovery while already running is a no-op. Stopping while not running is a no-op.

## Security Considerations

- **No warm connections**: Discovering a peer does NOT automatically establish an encrypted session. The "Connect" button initiates a fresh handshake.
- **Identity verification**: The first connection to a discovered peer is always "unverified." User must compare fingerprints out-of-band.
- **Rate limiting**: All incoming connections go through `ConnectionLimiter` regardless of discovery source.
- **Key store trust**: Only previously-verified peers get auto-trusted on reconnection.

## Test Plan

| Test | What it covers |
|---|---|
| SettingsContext test — LAN toggle | Calls `set_discovery_config` with `lan_enabled: true` |
| SettingsContext test — DHT toggle | Calls `set_discovery_config` with `dht_enabled: true` |
| SettingsContext test — both off | Default state, toggles work independently |
| Existing LAN unit tests | Packet format, parsing, expiry |
| Existing DHT unit tests | Message roundtrip, peer expiry |
