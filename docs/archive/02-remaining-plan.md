# Remaining Plan: M2M Secure Messenger

**Status**: Frontend UI and File Transfer logic remaining.
**Last Updated**: 2026-05-28

This document outlines the exact steps needed to complete the Minimum Viable Product (MVP) according to the original requirements and threat model constraints.

## 1. File and Image Transfer (Backend) - Phase 5
The protocol packet formats (`FileTransferRequest`, `FileTransferChunk`, `FileTransferComplete`, `FileTransferAccept`, `FileTransferReject`) are defined, but the operational logic needs to be implemented in the Rust backend.
- **Implement File Send logic**: Read a file from the disk, chunk it (max 256 KiB), hash each chunk, and send it over the encrypted TCP stream.
- **Implement File Receive logic**: Receive a transfer request, ask the user (via UI) to accept or reject it. If accepted, stream chunks to an encrypted directory in the `storage` module.
- **Security Check**: Enforce strict file size limits, verify chunk hashes, and ensure files are NEVER auto-opened or executed.

## 2. Tie Storage to Commands (Backend) - Finalizing Phase 7
Currently, the Tauri commands (`src-tauri/src/commands.rs`) store the `IdentityKeypair` and message states in memory (`AppState`).
- **Persistence**: We need to connect `commands.rs` to the `storage.rs` module so that on first launch, the generated identity is saved to `keys.db`.
- **Chat History**: When a message is sent or received, it needs to be written to `messages.db` using the application-level XChaCha20-Poly1305 encryption wrapper we built.

## 3. The React UI (Frontend) - Phase 6
The UI needs to be built from scratch (currently default Vite/React scaffold). It must feel premium, dark-mode preferred, dynamic, and strictly privacy-focused.

**Key UI Components Needed:**
1. **Welcome / Identity Setup View**:
   - Generates the keypair on first load.
   - Shows the user's generated identity fingerprint.
2. **Invite Exchange View**:
   - Button to generate and copy a signed invite link (`m2m://...`).
   - Input box to paste a peer's invite link and connect.
3. **Chat View**:
   - Real-time chat interface showing messages.
   - Security state indicators (Verified, Unverified, Disconnected, Reconnecting).
   - A clear "Fingerprint Verification" modal allowing users to manually compare fingerprints with their peer out-of-band.
4. **File Transfer View**:
   - UI to select files to send.
   - Incoming file prompt (Accept / Reject) so nothing downloads without permission.
   - Sandboxed/safe preview logic for downloaded images (using isolated Object URLs).

## Rules for Next Execution
- **Strict adherence to the threat model**: Do not bypass security checks to make things "work faster".
- **Aesthetics**: The React frontend must look premium and polished. Avoid generic styling.
- **Safety**: Do not add dependencies unless explicitly necessary. Keep the React footprint small and auditable.
