
---

### Deep‑Scan Observations (Read‑Only Inspection)

#### 1. Core Crypto (`src-tauri/src/crypto.rs`)
* **Implemented primitives** – Ed25519, X25519, XChaCha20‑Poly1305, HKDF, SHA‑256. All wrapped safely via `sodiumoxide`.
* **Error handling** – Comprehensive `CryptoError` enum; all public APIs return `Result`.
* **Tests** – Extensive unit tests covering Double Ratchet, X3DH, edge cases (missing chains, ratchet intervals). Tests compile in release builds (no `#[cfg(test)]` guard).
* **Gaps / Risks**
  * No **sender‑key** implementation for group chat (planned ~250 lines).
  * A handful of `panic!` calls in other modules (see section 2) – consider converting to proper errors for robustness.

#### 2. Session Layer (`src-tauri/src/session.rs`)
* Handles encrypted frame sending/receiving, ratchet integration, reconnection (`reconnect.rs`).
* **Borrow‑conflict** noted (lines 363, 400) – the guide already documents the correct destructuring pattern.
* **Panics** on unexpected packet types – replace with protocol‑error returns where possible.

#### 3. Protocol (`src-tauri/src/protocol.rs`)
* MessagePack serialization, `PacketType` enum.
* **Missing packet** – `TypingIndicator` (+60 lines) pending implementation.
* **Panics** on unknown packet variants (line 773).

#### 4. DHT & LAN Discovery
* Fully functional custom Kademlia‑style DHT and UDP multicast; no TODO/FIXME markers.

#### 5. Relay Server (`relay-server/src/main.rs`)
* Implements TURN/relay; builds via Docker compose. No apparent gaps.

#### 6. Storage (`src-tauri/src/storage.rs`)
* SQLite with WAL, appropriate indexes (`idx_messages_conversation`, `idx_messages_expires_at`, …). No outstanding TODOs.

#### 7. Commands
| File | Observations |
|------|--------------|
| `chat.rs` | Most messaging features present; search UI partially done, reaction UI missing; borrow conflict noted. |
| `network.rs` | Handles encrypted packet routing; reconnection integrated. |
| `vault.rs` | Identity export/import with Argon2id‑wrapped backup fully done. |
| `security.rs` | Clipboard clear, idle lock, screen‑capture protection implemented. |
| `relay.rs` | Contains a `panic!` on unexpected server errors (line 631). |

#### 8. Frontend (React/TSX)
* **ChatView** – Reaction badges & markdown rendering done; dark‑theme CSS missing; typing‑indicator UI absent.
* **HubView** – No per‑conversation full‑text search.
* **SetupView** – Only a loading spinner; onboarding tutorial missing.
* **VaultView** – Minor UX polish pending.
* **Styles** – No dark‑theme rules; theming system incomplete.
* **Keyboard shortcuts** – Partial (Esc, `Ctrl+,`, `?`); many common shortcuts missing.

#### 9. Types (`src/types.ts`)
* Needs updates for new fields: group IDs, sync tokens, typing state, search indices.

#### 10. Miscellaneous
* `Cargo.toml` lists optional deps (`qrcode`, `cpal`, `tauri-plugin-tray`, `tauri-plugin-updater`) but they are not enabled – required for pending features.
* **Panics** – 8 total across the Rust codebase (`identity.rs`, `relay.rs`, `secure_key.rs`, `protocol.rs`, `session.rs`). Consider converting to error returns for production robustness.
* No `unimplemented!` macros or `FIXME` comments in TypeScript files.

---

### Overall Health Summary
| Category | Status |
|----------|--------|
| Crypto & Security | ✅ Excellent |
| Peer Discovery (DHT/LAN) | ✅ Complete |
| Core Messaging (DR, X3DH) | ✅ Complete |
| Message UI Features (edit, delete, reactions, markdown) | ✅ Complete |
| Group Chat | ❌ Not started |
| Frontend Polish (dark theme, typing, shortcuts) | ⚠️ Partial |
| Multi‑Device Sync | ❌ Not started |
| System Tray / Background | ⚠️ Partial |
| Distribution (signing, updater) | ❌ Not started |
| Documentation / Onboarding | ⚠️ Partial |
| Tests | ✅ High coverage (panics remain in production code) |

### Recommended Priorities
1. **Group Chat** – implement sender‑key logic, new packet types, UI components, storage schema.
2. **Frontend Overhaul** – add dark‑theme CSS, typing‑indicator badge, full‑text search bar, keyboard shortcuts.
3. **Resolve Borrow Conflict** in `chat.rs` and replace remaining `panic!` calls with proper errors where feasible.
4. **Enable optional dependencies** (`tauri-plugin-tray`, `tauri-plugin-updater`, `qrcode`, `cpal`) once their associated features are coded.
5. **Finish Multi‑Device Sync** – design protocol, add `sync.rs`, extend storage and UI.
6. **Complete onboarding tutorial** in `SetupView` and create `docs/user-guide.md`.

Feel free to direct the next development focus or request a concrete implementation plan for any of the items above.
