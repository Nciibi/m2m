# M2M — UI/UX Design Bible (Part 3): Complete Error States, Loading Skeletons, Database Schema & Engineering Spec

**Version**: 3.0
**Coverage**: Every error message, every tooltip, every loading skeleton, every database table, every z-index, offline behavior, error boundary specs, state sync protocol, every user-facing string
**Extension of**: Parts 1 & 2

> This document exhaustively specifies every non-happy-path state in the application.
> Every error has exact text. Every tooltip has exact wording.
> Every loading state has a visual spec. Every database column is documented.
> No engineer should ever guess what to show the user.

---

## Table of Contents

21. [Complete Error Message Catalog](#21-complete-error-message-catalog)
22. [Complete Tooltip & Help Text Catalog](#22-complete-tooltip--help-text-catalog)
23. [Loading Skeleton Specifications](#23-loading-skeleton-specifications)
24. [Database Schema Specification](#24-database-schema-specification)
25. [Z-Index Map](#25-z-index-map)
26. [Offline & Degraded Mode Behavior](#26-offline--degraded-mode-behavior)
27. [Error Boundary Specifications](#27-error-boundary-specifications)
28. [State Sync Protocol](#28-state-sync-protocol)
29. [Complete User-Facing String Catalog](#29-complete-user-facing-string-catalog)
30. [Testing Matrix](#30-testing-matrix)
31. [Timer & Timeout Specifications](#31-timer--timeout-specifications)
32. [Focus Trap & Keyboard Edge Cases](#32-focus-trap--keyboard-edge-cases)

---

## 21. Complete Error Message Catalog

Every error in M2M has exact text, a type (error/warning/info), a display method (toast/inline/modal/badge), and a duration.

### 21.1 Vault Errors

| ID | Trigger | Message | Type | Display | Duration | Action |
|----|---------|---------|------|---------|----------|--------|
| V-001 | Passphrase < 12 chars | "Passphrase must be at least 12 characters." | error | inline (below input) | Until corrected | Field shake animation |
| V-002 | Passphrase mismatch (create) | "Passphrases do not match." | error | inline | Until corrected | Field shake |
| V-003 | Entropy < 40 bits | "Passphrase too weak: ~{bits} bits. Use longer (aim for 60+). Try a diceware phrase with 5+ random words." | error | inline | Until corrected | Field shake + strength bar red |
| V-004 | Argon2id derivation failure | "Failed to derive encryption key. The vault may be corrupted." | error | inline | 8s | Form remains, log error |
| V-005 | Decryption failure (wrong passphrase) | "Wrong passphrase. Please try again." | error | inline | Until corrected | Shake + clear input |
| V-006 | Identity key deserialization failure | "Failed to read identity key. The vault may be corrupted. If this persists, you may need to create a new identity." | error | inline | Until dismissed | Button changes to "Repair Vault" |
| V-007 | Key store open failure | "Could not open vault database: {path}. Check file permissions." | error | toast | 8s | Show file path |
| V-008 | Storage key zeroize failure | "Vault locked." | info | toast | 4s | Navigate to vault |
| V-009 | Export identity passphrase too short | "Export passphrase must be at least 12 characters." | error | inline | Until corrected | N/A |
| V-010 | Export identity file write error | "Failed to export identity: {error}" | error | toast | 8s | N/A |
| V-011 | Import identity file read error | "Failed to read identity file: {error}" | error | toast | 8s | N/A |
| V-012 | Import identity format error | "Invalid identity file format. Expected encrypted JSON." | error | toast | 8s | N/A |
| V-013 | Import identity decryption error | "Wrong passphrase or corrupted identity file." | error | toast | 8s | N/A |
| V-014 | Vault already unlocked | "Vault is already unlocked." | info | toast | 4s | N/A |

### 21.2 Connection Errors

| ID | Trigger | Message | Type | Display | Duration | Action |
|----|---------|---------|------|---------|----------|--------|
| C-001 | Invalid invite format | "Invalid invite link format. Expected 'm2m://...'" | error | inline (below input) | Until corrected | Clear field |
| C-002 | Invite expired | "This invite has expired. Ask the peer to generate a new one." | error | inline | Until corrected | Clear field |
| C-003 | Invite self-connect | "You cannot connect to yourself." | warning | inline | Until corrected | Clear field |
| C-004 | Connection timeout | "Connection timed out. The peer may be offline or behind a firewall." | error | toast | 8s | Reset connect button |
| C-005 | Connection refused | "Connection refused. The peer may not be listening." | error | toast | 8s | Reset connect button |
| C-006 | Handshake mismatch | "Protocol version mismatch. The peer may be running a different version." | error | toast | 8s | Disconnect |
| C-007 | Handshake timeout | "Handshake timed out. The peer may not be responding." | error | toast | 8s | Disconnect |
| C-008 | Reconnect attempt N/5 | "Reconnecting... (attempt {n}/5)" | info | badge | Until resolved | N/A |
| C-009 | Reconnect exhausted | "Reconnection failed after 5 attempts. The peer may be offline or your network may have changed. Please re-share an invite." | error | toast | 8s | Navigate to hub |
| C-010 | Reconnect success | "Reconnected to {displayName}" | success | toast | 4s | N/A |
| C-011 | Connection lost (unverified) | "Connection lost. Peer was not verified, so reconnection is not available." | warning | toast | 6s | Navigate to hub |
| C-012 | Connection lost (verified) | "Connection lost. Click Reconnect to attempt recovery, or re-share an invite." | warning | badge | Until action | Show reconnect button |

### 21.3 Message Errors

| ID | Trigger | Message | Type | Display | Duration | Action |
|----|---------|---------|------|---------|----------|--------|
| M-001 | Send while disconnected | "Cannot send message while disconnected." | warning | inline (above input) | Until reconnected | N/A |
| M-002 | Send failure (encryption) | "Failed to encrypt message. The session may need to be re-established." | error | toast | 6s | N/A |
| M-003 | Send failure (network) | "Failed to send message. It has been queued and will be delivered when the connection is restored." | warning | toast | 6s | N/A |
| M-004 | Message too long | "Message exceeds the 64KB limit. Please shorten it." | warning | inline (character count turns red) | Until shortened | Block send |
| M-005 | Edit not found | "Message could not be edited. It may have been deleted." | warning | toast | 5s | N/A |
| M-006 | Delete not found | "Message could not be deleted. It may have already been removed." | info | toast | 4s | N/A |
| M-007 | Edit after too long | "Messages can only be edited within 24 hours." | warning | toast | 5s | N/A |
| M-008 | Reaction send failed | "Failed to send reaction." | warning | toast | 4s | N/A |
| M-009 | Message load failed | "Failed to load messages." | warning | toast | 5s | Retry |
| M-010 | Search failed | "Search failed. Please try again." | warning | toast | 4s | N/A |
| M-011 | Export conversation failed | "Failed to export conversation: {error}" | error | toast | 6s | N/A |

### 21.4 File Transfer Errors

| ID | Trigger | Message | Type | Display | Duration | Action |
|----|---------|---------|------|---------|----------|--------|
| F-001 | File too large | "File exceeds the maximum transfer size." | error | toast | 6s | N/A |
| F-002 | File not found | "File not found at path: {path}" | error | toast | 6s | N/A |
| F-003 | File read error | "Failed to read file: {error}" | error | toast | 6s | N/A |
| F-004 | Transfer cancelled by peer | "{filename} was cancelled by the peer." | info | toast | 4s | Remove progress bar |
| F-005 | Transfer cancelled by self | "{filename} cancelled." | info | toast | 4s | Remove progress bar |
| F-006 | Chunk send timeout | "Chunk send timed out. Retrying ({n}/3)..." | warning | progress bar label | Until success/fail | Auto-retry |
| F-007 | Chunk hash mismatch | "File integrity check failed. The transfer may have been tampered with." | error | toast | 8s | Remove progress bar |
| F-008 | Transfer complete (sender) | "{filename} sent successfully." | success | toast | 4s | N/A |
| F-009 | Transfer complete (receiver) | "{filename} downloaded successfully." | success | toast | 4s | Offer to open file |
| F-010 | Accept failed | "Failed to accept file transfer: {error}" | error | toast | 6s | N/A |
| F-011 | Reject failed | "Failed to reject file transfer: {error}" | warning | toast | 4s | N/A |
| F-012 | Save dialog cancelled | "File save cancelled." | info | toast | 3s | N/A |
| F-013 | Disk write error | "Failed to save file: {error}. Check disk space and permissions." | error | toast | 8s | N/A |

### 21.5 Discovery Errors

| ID | Trigger | Message | Type | Display | Duration | Action |
|----|---------|---------|------|---------|----------|--------|
| D-001 | STUN discovery failure | "STUN discovery failed: {error}. Your IP address could not be determined." | warning | toast | 6s | N/A |
| D-002 | STUN all servers failed | "All STUN servers are unreachable. Some connection features may not work." | warning | toast | 6s | N/A |
| D-003 | LAN discovery toggle failed | "Failed to toggle LAN discovery: {error}" | error | toast | 5s | Revert toggle |
| D-004 | DHT discovery toggle failed | "Failed to toggle DHT discovery: {error}" | error | toast | 5s | Revert toggle |
| D-005 | No DHT bootstrap nodes | "No DHT bootstrap nodes configured. Discovery may not find peers." | warning | toast | 6s | N/A |
| D-006 | Connect to discovered peer failed | "Connection to discovered peer failed: {error}" | error | toast | 6s | N/A |
| D-007 | Peer list refresh failed | "Failed to refresh peer list." | warning | toast | 4s | N/A |

### 21.6 Settings Errors

| ID | Trigger | Message | Type | Display | Duration | Action |
|----|---------|---------|------|---------|----------|--------|
| S-001 | STUN server add invalid format | "Invalid STUN server address. Format: host:port (e.g., stun.example.com:3478)" | error | inline (below input) | Until corrected | N/A |
| S-002 | STUN server add empty | "STUN server cannot be empty." | warning | inline | Until corrected | N/A |
| S-003 | STUN server duplicate | "This STUN server is already in the list." | info | inline | 4s | N/A |
| S-004 | STUN remove last server | "Cannot remove the last STUN server — at least one is required." | warning | toast | 5s | N/A |
| S-005 | STUN reset to defaults | "STUN servers reset to defaults." | info | toast | 4s | N/A |
| S-006 | Tor toggle failed | "Failed to toggle Tor: {error}" | error | toast | 5s | Revert toggle |
| S-007 | Tor test timeout | "Tor test timed out. The Tor proxy may not be running." | warning | toast | 6s | N/A |
| S-008 | Tor test reachable | "Tor ✓ Proxy is reachable." | success | toast | 4s | N/A |
| S-009 | Tor test unreachable | "Tor ✗ Proxy is not reachable. Ensure Tor is running." | warning | toast | 6s | N/A |
| S-010 | Private mode toggle failed | "Failed to toggle private mode: {error}" | error | toast | 5s | Revert toggle |
| S-011 | Theme save failed | "Failed to save theme preference: {error}" | warning | toast | 4s | N/A |
| S-012 | Screen capture toggle failed | "Failed to toggle screen capture protection." | warning | toast | 4s | N/A |
| S-013 | Clipboard clear failed | "Failed to clear clipboard." | warning | toast | 4s | N/A |
| S-014 | Vault lock (from settings) | "Vault locked." | success | toast | 4s | Navigate to vault |
| S-015 | Vault lock failed | "Failed to lock vault: {error}" | error | toast | 5s | N/A |
| S-016 | Connectivity check reachable | "Connectivity: Reachable (NAT: {type})" | success | toast | 4s | N/A |
| S-017 | Connectivity check unreachable | "Connectivity: Not reachable. You may need a relay server." | warning | toast | 6s | N/A |

### 21.7 Security Errors

| ID | Trigger | Message | Type | Display | Duration | Action |
|----|---------|---------|------|---------|----------|--------|
| SEC-001 | Fingerprint verify confirm | "Peer verified. Always verify fingerprints via a trusted out-of-band channel." | success | toast + modal | 4s | Close modal |
| SEC-002 | Fingerprint mismatch | "Fingerprints do not match. Do NOT proceed with this peer." | error | modal | Until dismissed | Modal stays open |
| SEC-003 | Clipboard auto-clear set | "Clipboard will auto-clear in {n} seconds." | info | toast | 3s | N/A |
| SEC-004 | Clipboard cleared | "Clipboard cleared." | info | toast | 3s | N/A |
| SEC-005 | Screen capture protection enabled | "Screen capture protection enabled. Your window will not appear in screenshots." | info | toast | 4s | N/A |
| SEC-006 | Screen capture protection disabled | "Screen capture protection disabled." | info | toast | 4s | N/A |
| SEC-007 | Idle lock enabled | "Vault will auto-lock after {n} minutes of inactivity." | info | toast | 4s | N/A |
| SEC-008 | Vault auto-locked (idle) | "Vault auto-locked due to inactivity." | info | toast | 4s | Navigate to vault |

### 21.8 Group Chat Errors

| ID | Trigger | Message | Type | Display | Duration | Action |
|----|---------|---------|------|---------|----------|--------|
| G-001 | Create group insufficient members | "A group must have at least 2 members (including yourself)." | error | inline | Until corrected | N/A |
| G-002 | Create group encryption error | "Failed to create group encryption keys." | error | toast | 6s | N/A |
| G-003 | Group not found | "Group not found. It may have been deleted." | warning | toast | 5s | Navigate to hub |
| G-004 | Already a member | "You are already a member of this group." | info | toast | 4s | N/A |
| G-005 | Not a member | "You are not a member of this group." | warning | toast | 5s | N/A |
| G-006 | Leave group as admin with members | "You cannot leave as admin while other members are present. Transfer admin first or remove all members." | warning | modal | Until action | Show transfer admin option |
| G-007 | Remove member not admin | "Only admins can remove members." | warning | toast | 5s | N/A |
| G-008 | Send group message failed | "Failed to send group message." | warning | toast | 5s | N/A |
| G-009 | Load group messages failed | "Failed to load group messages." | warning | toast | 5s | N/A |
| G-010 | Invite to group failed | "Failed to invite member to group." | warning | toast | 5s | N/A |
| G-011 | Group name update failed | "Failed to update group name." | warning | toast | 5s | N/A |

### 21.9 System Errors

| ID | Trigger | Message | Type | Display | Duration | Action |
|----|---------|---------|------|---------|----------|--------|
| SYS-001 | Tauri IPC error | "Internal error: {error}. Please restart the application." | error | toast | 8s | Offer restart |
| SYS-002 | Database corruption | "Database error: {error}. Some features may not be available. Try restarting." | error | toast | 8s | Offer restart |
| SYS-003 | Memory warning | "The application is using high memory. Consider restarting." | warning | toast | 6s | N/A |
| SYS-004 | Update available | "Update v{version} is available. New features and security improvements included." | info | banner | Until dismissed/updated | Show "Update Now" button |
| SYS-005 | Update check failed | "Could not check for updates: {error}" | warning | toast | 5s | N/A |
| SYS-006 | Update download failed | "Update download failed: {error}" | error | toast | 6s | Retry |
| SYS-007 | Update installing | "Installing update v{version}..." | info | toast | Until complete | Progress bar |
| SYS-008 | Update installed | "Update installed. Restart to apply." | success | toast | 6s | "Restart Now" button |

### 21.10 First-Run/Onboarding Errors

| ID | Trigger | Message | Type | Display | Duration | Action |
|----|---------|---------|------|---------|----------|--------|
| O-001 | Key generation failure | "Failed to generate identity keys. This is a critical error. Please restart." | error | full-screen | Until restart | "Restart" button |
| O-002 | Key storage failure | "Failed to store identity keys. Check disk space and permissions." | error | full-screen | Until action | "Retry" button |
| O-003 | Network init failure | "Failed to initialize networking. Some features may not work." | warning | toast | 6s | N/A |
| O-004 | Database init failure | "Failed to initialize local database: {error}" | error | toast | 8s | N/A |

---

## 22. Complete Tooltip & Help Text Catalog

### 22.1 Button Tooltips

| Element | Tooltip Text | Position | Delay | Trigger |
|---------|-------------|----------|-------|---------|
| Settings gear | "Settings" | bottom | 500ms | hover |
| Back to Hub | "Back to conversations (Esc)" | bottom | 500ms | hover |
| Send message | "Send message (Ctrl+Enter)" | top | 500ms | hover |
| Attach file | "Attach a file" | top | 500ms | hover |
| Emoji picker | "Add emoji" | top | 500ms | hover |
| Disconnect | "Disconnect from peer" | bottom | 500ms | hover |
| Reconnect | "Attempt to reconnect" | bottom | 500ms | hover |
| Verify fingerprint | "Verify peer identity" | bottom | 500ms | hover |
| Copy invite | "Copy to clipboard" | top | 500ms | hover |
| Copy fingerprint | "Copy fingerprint" | top | 500ms | hover |
| Copy IP | "Copy IP address" | right | 500ms | hover |
| Generate invite | "Generate a one-time connection invite" | top | 500ms | hover |
| Connect to peer | "Connect using the pasted invite" | top | 500ms | hover |
| Scroll to bottom | "Scroll to latest messages" | left | 500ms | hover |
| Favorite star | "Add to favorites" / "Remove from favorites" | top | 500ms | hover |
| Archive folder | "Archive conversation" / "Unarchive" | top | 500ms | hover |
| Mute bell | "Mute notifications" / "Unmute notifications" | top | 500ms | hover |
| Delete trash | "Delete conversation" | top | 500ms | hover |
| Lock vault | "Lock vault — zeroize keys in memory" | top | 500ms | hover |
| Clear clipboard | "Clear clipboard contents now" | top | 500ms | hover |
| Test Tor | "Test if Tor proxy is reachable" | top | 500ms | hover |
| Discover STUN | "Discover your public IP address via STUN" | top | 500ms | hover |
| Check connectivity | "Run a connectivity check" | top | 500ms | hover |
| Refresh (discovery) | "Refresh peer list" | top | 500ms | hover |
| Reset (STUN) | "Reset to default STUN servers" | top | 500ms | hover |
| Copy public key | "Copy public key to clipboard" | top | 500ms | hover |
| Reset accent | "Reset to default accent color" | top | 500ms | hover |
| Close modal | "Close (Esc)" | top | 500ms | hover |
| Dismiss toast | "Dismiss" | top | immediate | hover |
| Update now | "Download and install the latest update" | bottom | 500ms | hover |
| Paste passphrase | "Paste from clipboard" | top | 500ms | hover |
| Show/hide passphrase | "Toggle passphrase visibility" | top | 500ms | hover |
| Clear input | "Clear" | top | 500ms | hover |
| Export conversation | "Export conversation history to file" | top | 500ms | hover |
| Accept file | "Accept and download file" | top | 500ms | hover |
| Reject file | "Reject file transfer" | top | 500ms | hover |
| Self-destruct timer | "Set message self-destruct timer" | top | 500ms | hover |
| Open image | "Open image" | top | 500ms | hover |

### 22.2 Help Text & Descriptions

| Location | Text | Purpose |
|----------|------|---------|
| Vault description (create) | "Choose a strong passphrase to encrypt your identity keys and message history." | Explain purpose |
| Vault description (unlock) | "Enter your passphrase to decrypt your local data." | Explain purpose |
| Vault hint | "Minimum 12 chars · Argon2id" | Show requirements |
| Vault tips toggle | "What makes a strong passphrase?" | Toggle help |
| Vault tips content | "Use 5+ random words (diceware method). Aim for 60+ bits of entropy. Avoid common phrases or song lyrics. Include a mix of cases, numbers, or symbols. 'correct-horse-battery-staple' style is excellent." | Help text |
| Connect tab description (host) | "Generate a one-time signed invite for a peer to connect to you securely." | Explain card |
| Connect tab description (join) | "Paste an invite link from a trusted peer to connect." | Explain card |
| Setup description | "Generating Ed25519 identity keys. They never leave your device." | Explain process |
| Chat empty state | "Send a message below to begin your encrypted conversation. All messages are protected with end-to-end encryption." | Guide user |
| Chat footer | "End-to-end encrypted" | Security reassurance |
| Chat footer | "Ctrl+Enter to send · Esc to go back" | Keyboard hints |
| Session banner | "End-to-end encrypted session established." | Security reassurance |
| Discovery privacy notice | "Both are OFF by default for privacy. When enabled, your IP address is visible to observers on the discovery channel. Ephemeral IDs are used (not your permanent identity key) and rotate periodically." | Privacy explanation |
| Tor inbound warning | "Tor is enabled for outbound connections, but this invite contains your real IP address." | Security warning |
| Fingerprint modal description | "Compare fingerprints via a secure out-of-band channel (e.g., Signal, in-person, or phone call)." | Security instruction |
| Nearby empty (discovery off) | "Enable LAN or DHT discovery in Settings to find nearby peers." | Guide user |
| Nearby empty (no peers) | "No LAN peers detected. Make sure other M2M users are on the same network." | Explanation |
| Chats empty | "Generate an invite link to host a connection, or paste an invite from a peer to join." | Guide user |
| Chats search empty | "Try adjusting your search terms or clear the filter." | Guide user |

### 22.3 Placeholder Text

| Input | Placeholder |
|-------|-------------|
| Vault passphrase | "Passphrase" |
| Vault confirm | "Confirm passphrase" |
| Message input | "Type a secure message…" |
| Search conversations | "Search conversations…" |
| Search messages (ChatView) | "Search messages… (press Esc to close)" |
| Invite to connect | "m2m://..." |
| Your display name | "Your name (how they see you)" |
| Their display name | "Their name (how you see them)" |
| STUN server input | "host:port" |
| Group name (future) | "Group name" |
| Add family member | "Peer key hex" |
| Family nickname | "Nickname" |
| Export passphrase | "Export passphrase" |

---

## 23. Loading Skeleton Specifications

### 23.1 Conversation List Skeleton

```
┌──────────────────────────────────────────┐
│  ┌──────┐                                │
│  │ ░░░░ │  ░░░░░░░░░░░░░░░░░   ░░░░░░░  │  ← shimmer animation
│  │ ░░░░ │  ░░░░░░░░░░░░░░░░░            │     height: 64px
│  │ 48px │                                │     gap: 8px
│  └──────┘                                │
│  ─────────────────────────────────────── │
│  ┌──────┐                                │
│  │ ░░░░ │  ░░░░░░░░░░░░░░░░░   ░░░░░░░  │
│  │ ░░░░ │  ░░░░░░░░░░░░░░░░░            │
│  └──────┘                                │
│  ─────────────────────────────────────── │
│  ┌──────┐                                │
│  │ ░░░░ │  ░░░░░░░░░░░░░░░░░   ░░░░░░░  │
│  │ ░░░░ │  ░░░░░░░░░░░░░░░░░            │
│  └──────┘                                │
└──────────────────────────────────────────┘
```

**Specs**:
- 3 skeleton items, matching conversation item dimensions (64px height)
- Avatar: 48×48px rounded rectangle, `--radius-lg`
- Lines: full-width shimmer bars, `--radius-sm`
- Shimmer animation: `@keyframes shimmer` 2s linear infinite
- Shimmer colors: dark mode = `rgba(255,255,255,0.03)` → `rgba(255,255,255,0.08)` → `rgba(255,255,255,0.03)`
- Light mode: `rgba(0,0,0,0.03)` → `rgba(0,0,0,0.08)` → `rgba(0,0,0,0.03)`

### 23.2 Message List Skeleton

```
┌──────────────────────────────────────────┐
│                                         │
│  ┌─── Session Banner Skeleton ───────┐  │
│  │  🔒 ░░░░░░░░░░░░░░░░░░░░░░░░░░░  │  │
│  │     ░░░░░░░░░░░░░░░░░░░░░░░░░░░  │  │
│  └────────────────────────────────────┘  │
│                                         │
│  ─── ░░░░ ───                           │
│                                         │
│                    ┌──────────────────┐  │
│                    │  ░░░░░░░░░░░░░░  │  │  ← sent skeleton (right)
│                    │  ░░░░░░░░░░░░░░  │  │     max-width: 60%
│                    │  ░░░ ░░░░░      │  │     height: 52px
│                    └──────────────────┘  │
│                                         │
│  ┌──────────────────┐                    │
│  │  ░░░░░░░░░░░░░░  │                    │  ← received skeleton (left)
│  │  ░░░░░░░░░░░░░░  │                    │     max-width: 50%
│  │  ░░░ ░░░░░      │                    │     height: 52px
│  └──────────────────┘                    │
│                                         │
│                    ┌──────────────────┐  │
│                    │  ░░░░░░░░░░░░░░  │  │
│                    │  ░░░░░░░░░░░░░░  │  │
│                    │  ░░░ ░░░░░      │  │
│                    └──────────────────┘  │
└──────────────────────────────────────────┘
```

**Specs**:
- Session banner: full-width shimmer bar, height 60px
- Date separator: short shimmer line, centered
- Message bubbles: alternating sent/received skeletons
- Each skeleton bubble: similar dimensions to real bubbles
- Same shimmer animation as conversation list

### 23.3 Settings Skeleton

```
┌──────────────────────────────────────────┐
│  ─── ░░░░░░░ ───                         │
│  ┌────────────────────────────────────┐  │
│  │  ░░░░░░░░░░    ░░░░░░░░░░░░░░░░░  │  │
│  │  ░░░░░░░░░░    ░░░░░░░░░░░░░░░░░  │  │
│  └────────────────────────────────────┘  │
│                                         │
│  ─── ░░░░░░░ ───                         │
│  ┌────────────────────────────────────┐  │
│  │  ░░░░░░░░░░    ░░░░░░░░░░░░░░░░░  │  │
│  │  ░░░░░░░░░░    ░░░░░░░░░░░░░░░░░  │  │
│  │  ░░░░░░░░░░    ░░░░░░░░░░░░░░░░░  │  │
│  └────────────────────────────────────┘  │
└──────────────────────────────────────────┘
```

- Section title: short shimmer, 24px above card
- Card: `--radius-lg`, full-width, 3 rows of label + value shimmer
- 2-3 skeleton cards loaded

### 23.4 Loading State Rules

| State | Skeleton Type | Duration | Transition |
|-------|--------------|----------|------------|
| Initial app load | None (SetupView handles this) | N/A | N/A |
| Conversation list loading | Conv list skeleton (3 items) | Until loaded (max 3s) | Fade out 200ms |
| Messages loading | Message list skeleton (3 bubbles) | Until loaded (max 3s) | Fade out 200ms |
| Settings loading | Settings skeleton (2 cards) | Until loaded (max 3s) | Fade out 200ms |
| Search loading | No skeleton — spinner in search bar | Until results return | N/A |
| File transfer | No skeleton — progress bar | Until complete | N/A |
| Image loading | Pulsing placeholder square | Until decoded | Fade in image 300ms |
| User avatar loading | Pulsing circle with first letter | Until data loads | N/A |

---

## 24. Database Schema Specification

### 24.1 keys.db — Key Store

**Table: `identity`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Row identifier |
| `public_key` | BLOB | NOT NULL | Ed25519 public key (32 bytes) |
| `encrypted_secret_key` | BLOB | NOT NULL | Ed25519 secret key encrypted with storage key (112 bytes: nonce+encrypted) |
| `content_nonce` | BLOB | NOT NULL | XChaCha20-Poly1305 nonce (24 bytes) |
| `created_at` | INTEGER | NOT NULL | Unix timestamp of creation |
| `vault_initialized` | INTEGER | DEFAULT 0 | Whether vault passphrase has been set |

**Table: `peer_keys`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Row identifier |
| `public_key_hex` | TEXT | NOT NULL UNIQUE | Peer's Ed25519 public key as hex string |
| `encrypted_secret_key` | BLOB | NULL | Our signed prekey for this peer (encrypted) |
| `alias` | TEXT | NULL | User-set display name for this peer |
| `first_seen` | INTEGER | NOT NULL | Unix timestamp first seen |
| `last_seen` | INTEGER | NULL | Unix timestamp last seen |

**Table: `used_nonces`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Row identifier |
| `nonce` | BLOB | NOT NULL UNIQUE | Used XChaCha20-Poly1305 nonce |

**Table: `family`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `public_key_hex` | TEXT | PRIMARY KEY | Family member's Ed25519 public key |
| `nickname` | TEXT | NULL | User-set nickname |
| `added_at` | INTEGER | NOT NULL | Unix timestamp added |
| `expires_at` | INTEGER | NULL | Optional expiration timestamp |
| `last_address` | TEXT | NULL | Last known address |

**Table: `settings`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `key` | TEXT | PRIMARY KEY | Setting name |
| `value` | TEXT | NOT NULL | Setting value (JSON-encoded) |

### 24.2 messages.db — Message Store

**Table: `conversations`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | TEXT | PRIMARY KEY | Conversation identifier (peer_key_hex) |
| `peer_id` | BLOB | NOT NULL | Peer identity public key |
| `created_at` | INTEGER | NOT NULL | Unix timestamp of first message |
| `last_message_at` | INTEGER | NULL | Unix timestamp of most recent message |
| `display_name` | TEXT | NULL | User-set display name for this conversation |
| `peer_display_name` | TEXT | NULL | Peer's display name for themselves |
| `auto_delete_at` | INTEGER | NULL | Unix timestamp for auto-deletion |
| `retention_policy` | TEXT | DEFAULT 'none' | Policy: 'none', 'delete', 'export' |
| `is_favorite` | INTEGER | DEFAULT 0 | Whether conversation is favorited |
| `archived` | INTEGER | DEFAULT 0 | Whether conversation is archived |

**Indexes**:
```sql
CREATE INDEX idx_conversations_last_message ON conversations(COALESCE(last_message_at, created_at) DESC);
CREATE INDEX idx_conversations_favorite ON conversations(is_favorite DESC) WHERE is_favorite = 1;
```

**Table: `messages`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | TEXT | PRIMARY KEY | Unique message ID (UUID v4) |
| `conversation_id` | TEXT | NOT NULL REFERENCES conversations(id) ON DELETE CASCADE | Parent conversation |
| `direction` | TEXT | NOT NULL | 'sent' or 'received' |
| `content_encrypted` | BLOB | NOT NULL | XChaCha20-Poly1305 encrypted message content |
| `content_nonce` | BLOB | NOT NULL | Nonce for content encryption |
| `timestamp` | INTEGER | NOT NULL | Unix timestamp of message |
| `read_at` | INTEGER | NULL | Unix timestamp when message was read |
| `edited_at` | INTEGER | NULL | Unix timestamp when message was last edited |
| `deleted` | INTEGER | DEFAULT 0 | Soft-delete flag |
| `expires_at` | INTEGER | NULL | Unix timestamp for self-destruct |

**Indexes**:
```sql
CREATE INDEX idx_messages_conversation ON messages(conversation_id, timestamp DESC);
CREATE INDEX idx_messages_expires_at ON messages(expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX idx_messages_read_at ON messages(conversation_id, read_at) WHERE read_at IS NULL;
```

**Table: `reactions`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Row identifier |
| `message_id` | TEXT | NOT NULL REFERENCES messages(id) ON DELETE CASCADE | Target message |
| `emoji` | TEXT | NOT NULL | Reaction emoji string |
| `peer_key_hex` | TEXT | NOT NULL | Peer who reacted |
| `timestamp` | INTEGER | NOT NULL | Unix timestamp |

**Index**: `CREATE INDEX idx_reactions_message ON reactions(message_id);`

**Table: `conversation_names`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Row identifier |
| `conversation_id` | TEXT | NOT NULL | Conversation identifier |
| `peer_key_hex` | TEXT | NOT NULL | Peer whose name this is |
| `name` | TEXT | NOT NULL | Display name |

**Table: `group_messages`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | TEXT | PRIMARY KEY | Unique message ID |
| `group_id` | TEXT | NOT NULL | Group identifier |
| `sender_peer_key_hex` | TEXT | NOT NULL | Sender's public key hex |
| `content_encrypted` | BLOB | NOT NULL | Encrypted content |
| `content_nonce` | BLOB | NOT NULL | Nonce |
| `timestamp` | INTEGER | NOT NULL | Unix timestamp |
| `delivered` | INTEGER | DEFAULT 0 | Delivery status |
| `edited_at` | INTEGER | NULL | Edit timestamp |
| `deleted` | INTEGER | DEFAULT 0 | Soft-delete |

**Table: `groups`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `group_id` | TEXT | PRIMARY KEY | Group identifier |
| `group_name` | TEXT | NOT NULL | Display name |
| `created_at` | INTEGER | NOT NULL | Unix timestamp |
| `last_message_at` | INTEGER | NULL | Unix timestamp of most recent message |
| `last_message_preview` | TEXT | NULL | Preview of most recent message |

**Table: `group_members`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `group_id` | TEXT | NOT NULL REFERENCES groups(group_id) ON DELETE CASCADE | Group identifier |
| `peer_key_hex` | TEXT | NOT NULL | Member's public key hex |
| `display_name` | TEXT | NULL | Nickname in group |
| `role` | TEXT | NOT NULL DEFAULT 'member' | 'admin' or 'member' |
| `added_at` | INTEGER | NOT NULL | Unix timestamp added |

### 24.3 transfers.db — Transfer Store

**Table: `transfers`**
| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Row identifier |
| `transfer_id` | TEXT | NOT NULL UNIQUE | UUID of this transfer |
| `peer_key_hex` | TEXT | NOT NULL | Peer involved |
| `filename` | TEXT | NOT NULL | Original filename |
| `total_size` | INTEGER | NOT NULL | File size in bytes |
| `direction` | TEXT | NOT NULL | 'sent' or 'received' |
| `state` | TEXT | NOT NULL DEFAULT 'pending' | Transfer state machine value |
| `local_path` | TEXT | NULL | Saved file path (receiver) or source path (sender) |
| `chunks_acked` | INTEGER | DEFAULT 0 | Number of chunks acknowledged |
| `chunks_total` | INTEGER | NOT NULL | Total number of chunks |
| `created_at` | INTEGER | NOT NULL | Unix timestamp |
| `completed_at` | INTEGER | NULL | Unix timestamp when completed |
| `speed_bytes_per_sec` | INTEGER | NULL | Average speed |
| `error` | TEXT | NULL | Error message if failed |

**Indexes**: `CREATE INDEX idx_transfers_peer ON transfers(peer_key_hex);`

---

## 25. Z-Index Map

Every positioned element in the application must use one of these z-index values.

```css
:root {
  --z-base: 1;           /* Base stacking context — app shell content */
  --z-sticky: 10;         /* Sticky elements — headers, tab bars */
  --z-fab: 50;            /* Floating action buttons — scroll-to-bottom FAB */
  --z-drop-zone: 50;      /* Drag-and-drop overlay */
  --z-dropdown: 100;      /* Dropdowns, popovers, context menus */
  --z-tooltip: 200;       /* Tooltips */
  --z-toast: 1000;        /* Toast notifications */
  --z-modal: 9999;        /* Modal backdrops and content */
  --z-update-banner: 1000; /* Update notification banner (same as toast) */
}
```

**Element z-index assignments**:

| Element | z-index | Context |
|---------|---------|---------|
| `.app-shell` | auto (1) | Base content |
| `.app-header` | `--z-sticky` (10) | Sticky at top |
| `.tab-bar` | `--z-sticky` (10) | Below header |
| `.scroll-fab` | `--z-fab` (50) | Above messages |
| `.drop-zone--active::after` | `--z-drop-zone` (50) | Above input area |
| `.drop-zone__hint` | `--z-drop-zone` + 1 (51) | Above overlay |
| `.reaction-picker` | `--z-dropdown` (100) | Above messages |
| `.msg-context-menu` | `--z-dropdown` (100) | Above messages |
| `.emoji-picker-dropdown` | `--z-dropdown` (100) | Above input |
| `.conv-actions` | `--z-dropdown` (100) | Above conversation items |
| `.tooltip` | `--z-tooltip` (200) | Above everything in view |
| `.toast-container` | `--z-toast` (1000) | Above all content |
| `.update-banner` | `--z-toast` (1000) | Same level as toasts |
| `.modal-backdrop` | `--z-modal` (9999) | Above everything |
| `.modal-content` | `--z-modal` (9999) | Above backdrop |

---

## 26. Offline & Degraded Mode Behavior

### 26.1 Network Disconnected

**Trigger**: TCP connection drops, no active session.

**UI changes**:
- Header badge changes to "disconnected" (red/danger)
- Input area becomes disabled with message "Cannot send while disconnected"
- If peer was verified: "Reconnect" button appears in header
- If peer was unverified: Navigate to Hub automatically

**Data handling**:
- Outgoing messages are stored to the offline queue (in-memory + DB)
- Messages queued but not sent show ⏳ status
- On reconnect: flush queue via `flush_offline_queue()`
- Request missed messages via `SyncRequest` packet

**Queued message limit**: 500 messages max in offline queue.

### 26.2 Vault Locked

**Trigger**: User locks vault, idle lock fires, or app restart.

**UI changes**:
- Navigate to VaultView
- All sensitive data removed from memory
- Active connections remain open (Tauri decision) but cannot send
- Conversations list unavailable until unlock

### 26.3 Database Unavailable

**Trigger**: File system error, disk full, permission denied.

**UI changes**:
- Toast: "Database error. Some features may not be available."
- Conversations list shows empty state
- Settings still accessible
- "Retry" button to reinitialize stores

### 26.4 Peer Offline

**Trigger**: Peer not reachable, not responding.

**Behavior**:
- Connect attempt shows timeout after 15s
- Messages are queued for delivery on next connection
- Conversations show offline indicator (○)
- No auto-reconnect — user-initiated only

### 26.5 Cryptographic Key Error

**Trigger**: Key deserialization failure, ratchet desync.

**UI changes**:
- Session invalidated
- Toast: "Session error. A new connection must be established."
- Navigate to Hub
- User must re-invite or accept new invite

---

## 27. Error Boundary Specifications

### 27.1 Error Boundary Component Spec

Each Tauri view (SetupView, VaultView, HubView, ChatView, SettingsView) is wrapped in an `ErrorBoundary`.

**ErrorBoundary render states**:

**State 1 — No error (default)**:
```
  Renders children normally
```

**State 2 — Caught error**:
```
┌──────────────────────────────────────────┐
│  ⚠️ Something went wrong                  │
│                                         │
│  [view name] encountered an unexpected   │
│  error. The application can continue.    │
│                                         │
│  [error message]                         │  ← collapsed by default
│  [▼ Error details]                       │  ← expandable
│                                         │
│  [Dismiss]  [Reload View]  [Restart App] │
└──────────────────────────────────────────┘
```

**Specs**:
- Background: `--color-bg-elevated`, centered in view
- Icon: AlertTriangle, 48px, `--color-warning`
- Title: "Something went wrong", `--text-xl`, 700 weight
- Description: "[View name] encountered an unexpected error..."
- Error details: Expandable `<details>` with monospace error text
- Buttons: Dismiss (closes error boundary, returns to hub), Reload View (remounts), Restart App (invokes restart)

**State 3 — Fatal error (app-level)**:
```
┌──────────────────────────────────────────┐
│  ❌ Critical Error                        │
│                                         │
│  The application encountered a critical  │
│  error and cannot continue safely.       │
│                                         │
│  [error details]                         │
│                                         │
│  [Copy Error Log]  [Restart App]        │
└──────────────────────────────────────────┘
```

### 27.2 Error Boundary Coverage

| View | Error Boundary Level |
|------|---------------------|
| SetupView | Per-view |
| VaultView | Per-view |
| HubView | Per-view (with child boundaries for tabs) |
| ChatView | Per-view |
| SettingsView | Per-view |
| Toast system | Global (toast failures don't crash app) |
| Tauri IPC call | Per-invoke catch |

---

## 28. State Sync Protocol

### 28.1 Event Flow Map

Every state change in the frontend follows one of these patterns:

**Pattern A — User action → Backend → Event → UI update**:
```
User clicks button
  → invoke("tauri_command", args)
    → Rust handler processes
      → emits event("m2m://event-name", payload)
        → listen handler updates state
          → React re-renders
```

**Pattern B — Backend event → UI update** (incoming data):
```
Peer sends data over TCP
  → Rust receive loop decodes
    → emits event("m2m://event-name", payload)
      → listen handler updates state
        → React re-renders
```

**Pattern C — Optimistic UI → Backend → Correction**:
```
User performs action
  → UI updates immediately (optimistic)
    → invoke("tauri_command", args)
      → Rust handler processes
        → emits event("m2m://event-name", corrected payload)
          → listen handler overwrites optimistic state
```

### 28.2 Event Registry

| Event Name | Direction | Payload | When Fired |
|-----------|-----------|---------|------------|
| `m2m://message` | Backend → Frontend | `{ peer_key_hex, message: ChatMessage }` | Incoming message from peer |
| `m2m://connection` | Backend → Frontend | `{ peer_key_hex, state, peer_fingerprint, peer_verified }` | Connection state change |
| `m2m://file-request` | Backend → Frontend | `{ peer_key_hex, transfer_id, filename, total_size }` | Incoming file transfer request |
| `m2m://file-complete` | Backend → Frontend | `{ transfer_id, filename, path }` | File transfer completed |
| `m2m://transfer-progress` | Backend → Frontend | `TransferProgressEvent` | Chunk ACK received |
| `m2m://transfer-cancelled` | Backend → Frontend | `{ transfer_id, filename }` | Transfer cancelled |
| `m2m://conversation-meta` | Backend → Frontend | `{}` (refresh trigger) | Conversation metadata changed |
| `m2m://reaction` | Backend → Frontend | `{ message_id, reaction, peer_key_hex, remove }` | Reaction received/removed |
| `m2m://edit` | Backend → Frontend | `{ message_id, new_content, edited_at }` | Message edited by peer |
| `m2m://delete` | Backend → Frontend | `{ message_id }` | Message deleted by peer |
| `m2m://reconnect-attempt` | Backend → Frontend | `{ peer_key_hex, attempt, state }` | Reconnect progress |
| `m2m://typing` | Backend → Frontend | `{ peer_key_hex, typing: bool }` | Typing indicator |
| `m2m://group-message` | Backend → Frontend | `{ group_id, message: ChatMessage }` | Group message received |
| `m2m://group-event` | Backend → Frontend | `{ group_id, event_type, peer_key_hex }` | Group state change |

### 28.3 Invoke Command Registry

| Command | Args | Returns | Description |
|---------|------|---------|-------------|
| `init_identity` | — | `IdentityInfo` | Initialize identity keypair |
| `get_identity` | — | `IdentityInfo` | Get current identity info |
| `unlock_vault` | `passphrase` | `VaultStatus` | Unlock vault with passphrase |
| `get_vault_status` | — | `VaultStatus` | Check vault state |
| `lock_vault` | — | `()` | Lock vault |
| `is_first_run` | — | `bool` | Check first-run status |
| `set_first_run_complete` | — | `()` | Mark onboarding complete |
| `export_identity` | `passphrase` | `String` | Export identity JSON |
| `import_identity` | `payload, passphrase` | `IdentityInfo` | Import identity from JSON |
| `send_message` | `peerKeyHex, content` | `ChatMessage` | Send encrypted message |
| `send_message_with_timer` | `peerKeyHex, content, disappearAfter` | `ChatMessage` | Send with self-destruct |
| `edit_message` | `peerKeyHex, messageId, newContent` | `ChatMessage` | Edit sent message |
| `delete_message` | `peerKeyHex, messageId` | `()` | Delete message |
| `load_messages` | `peerKeyHex, beforeTimestamp?, limit?` | `ChatMessage[]` | Load messages |
| `search_messages` | `peerKeyHex, query` | `ChatMessage[]` | Search messages |
| `send_reaction` | `peerKeyHex, messageId, reaction` | `()` | Add reaction |
| `remove_reaction` | `peerKeyHex, messageId, reaction` | `()` | Remove reaction |
| `mark_messages_read` | `conversationId` | `()` | Mark conversation read |
| `send_typing_indicator` | `peerKeyHex, typing` | `()` | Send typing status |
| `list_conversations` | — | `ConversationListItem[]` | List conversations |
| `delete_conversation_cmd` | `conversationId` | `()` | Delete conversation |
| `toggle_favorite` | `peerKeyHex` | `bool` | Toggle favorite |
| `toggle_archive` | `peerKeyHex` | `bool` | Toggle archive |
| `mute_conversation` | `peerKeyHex` | `()` | Mute notifications |
| `unmute_conversation` | `peerKeyHex` | `()` | Unmute notifications |
| `get_muted_conversations` | — | `string[]` | Get muted list |
| `export_conversation` | `conversationId, exportPath` | `()` | Export to JSON |
| `send_file` | `peerKeyHex, filePath` | `()` | Start file transfer |
| `accept_file_transfer` | `peerKeyHex, transferId, saveDir` | `()` | Accept incoming file |
| `reject_file_transfer` | `peerKeyHex, transferId` | `()` | Reject file |
| `cancel_file_transfer` | `peerKeyHex, transferId` | `()` | Cancel file |
| `handle_sync_device_info` | `peerKeyHex, syncDeviceInfoData` | `()` | Sync device info |
| `handle_sync_payload` | `peerKeyHex, syncPayloadData` | `()` | Sync payload |
| `create_group` | `name, members` | `GroupInfo` | Create group |
| `send_group_message` | `groupId, content, timer?` | `ChatMessage` | Send group message |
| `list_groups` | — | `GroupInfo[]` | List groups |
| `get_group_info` | `groupId` | `GroupDetail` | Get group details |
| `invite_to_group` | `groupId, peerKeyHex` | `()` | Invite to group |
| `remove_from_group` | `groupId, peerKeyHex` | `()` | Remove from group |
| `leave_group` | `groupId` | `()` | Leave group |
| `load_group_messages` | `groupId, limit, offset` | `ChatMessage[]` | Load group messages |
| `update_group_name` | `groupId, name` | `()` | Rename group |
| `discover_public_ip` | — | `String` | STUN discovery |
| `get_stun_config` | — | `StunConfig` | Get STUN config |
| `set_stun_servers` | `servers` | `()` | Set STUN servers |
| `set_private_mode` | `enabled` | `()` | Toggle private mode |
| `set_tor_enabled` | `enabled` | `()` | Toggle Tor |
| `check_connectivity` | — | `ConnectivityStatus` | Run connectivity check |
| `get_network_diagnostics` | — | `NetworkDiagnostics` | Get diagnostics |
| `get_theme_preference` | — | `ThemePreference` | Get theme + accent |
| `set_theme_preference` | `theme, accent_color?` | `()` | Set theme + accent |
| `get_security_config` | — | `SecurityConfig` | Get security config |
| `set_security_config` | `config` | `SecurityConfig` | Set security config |
| `clear_clipboard` | — | `()` | Clear clipboard |

---

## 29. Complete User-Facing String Catalog

### 29.1 Navigation Strings

| Key | String | Context |
|-----|--------|---------|
| `nav.title` | "M2M" | App header title |
| `nav.hub` | "Hub" | Back to hub button |
| `nav.settings` | "Settings" | Settings header |
| `nav.tab.connect` | "Connect" | Connect tab label |
| `nav.tab.chats` | "Chats" | Chats tab label |
| `nav.tab.nearby` | "Nearby" | Nearby tab label |
| `nav.tab.family` | "Family" | Family tab label |
| `nav.back` | "Back" | Generic back button |

### 29.2 Vault Strings

| Key | String |
|-----|--------|
| `vault.title.create` | "Set Up Your Vault" |
| `vault.title.unlock` | "Unlock Your Vault" |
| `vault.desc.create` | "Choose a strong passphrase to encrypt your identity keys and message history." |
| `vault.desc.unlock` | "Enter your passphrase to decrypt your local data." |
| `vault.hint` | "Minimum 12 chars · Argon2id" |
| `vault.input.passphrase` | "Passphrase" |
| `vault.input.confirm` | "Confirm passphrase" |
| `vault.button.create` | "Create Vault" |
| `vault.button.unlock` | "Unlock" |
| `vault.match` | "Passphrases match" |
| `vault.mismatch` | "Passphrases do not match" |
| `vault.error.short` | "Passphrase must be at least 12 characters." |
| `vault.error.weak` | "Passphrase too weak: ~{bits} bits. Use longer (aim for 60+)." |
| `vault.error.wrong` | "Wrong passphrase. Please try again." |
| `vault.error.corrupt` | "Vault may be corrupted. If this persists, create a new identity." |
| `vault.tips.toggle` | "What makes a strong passphrase?" |
| `vault.tips.title` | "Tips:" |
| `vault.tips.1` | "Use 5+ random words (diceware method)" |
| `vault.tips.2` | "Aim for 60+ bits of entropy" |
| `vault.tips.3` | "Avoid common phrases or song lyrics" |
| `vault.tips.4` | "Include a mix of cases, numbers, or symbols" |
| `vault.tips.5` | '"correct-horse-battery-staple" style is excellent' |
| `vault.fingerprint_hint` | "This vault belongs to {fingerprint}…" |

### 29.3 Setup/Onboarding Strings

| Key | String |
|-----|--------|
| `setup.loading.title` | "Initializing Secure Enclave" |
| `setup.loading.desc` | "Generating Ed25519 identity keys.\nThey never leave your device." |
| `setup.loading.crypto` | "Ed25519 · X25519 · XChaCha20-Poly1305" |
| `onboarding.step1.title` | "Welcome to M2M" |
| `onboarding.step1.desc` | "A private, end-to-end encrypted messenger. No servers, no accounts, no tracking." |
| `onboarding.step1.icon` | "🚀" |
| `onboarding.step2.title` | "Your Identity is Local" |
| `onboarding.step2.desc` | "Your keys are generated on this device and never leave it." |
| `onboarding.step2.icon` | "🔑" |
| `onboarding.step3.title` | "End-to-End Encrypted" |
| `onboarding.step3.desc` | "Messages use X3DH + Double Ratchet (Signal protocol). Ed25519 signing, X25519 key exchange, XChaCha20-Poly1305 encryption." |
| `onboarding.step3.icon` | "🔒" |
| `onboarding.step4.title` | "Ready to Go!" |
| `onboarding.step4.desc` | "Share your invite link with a trusted peer to start chatting. Both sides must generate and share invites." |
| `onboarding.step4.icon` | "✅" |
| `onboarding.button.start` | "Get Started" |
| `onboarding.button.next` | "Next" |
| `onboarding.button.back` | "Back" |
| `onboarding.button.finish` | "Start Messaging" |

### 29.4 Connection Strings

| Key | String |
|-----|--------|
| `connect.host.title` | "Host a Connection" |
| `connect.host.desc` | "Generate a one-time signed invite for a peer to connect to you securely." |
| `connect.host.button` | "Generate Invite Link" |
| `connect.host.listening` | "Listening for incoming connections" |
| `connect.host.expires` | "Expires in {m}:{s}" |
| `connect.host.recent` | "Recent Invites" |
| `connect.join.title` | "Join a Connection" |
| `connect.join.desc` | "Paste an invite link from a trusted peer to connect." |
| `connect.join.input` | "m2m://..." |
| `connect.join.button` | "Connect" |
| `connect.join.valid` | "Valid Invite Found" |
| `connect.join.name.you` | "Your Name" |
| `connect.join.name.hint_you` | "How they will see you" |
| `connect.join.name.them` | "Their Name" |
| `connect.join.name.hint_them` | "How you want to see them" |
| `connect.fingerprint.label` | "Your Identity Fingerprint" |
| `connect.fingerprint.copy` | "Copy fingerprint" |

### 29.5 Conversation Strings

| Key | String |
|-----|--------|
| `chats.search` | "Search conversations…" |
| `chats.empty.title` | "No conversations yet" |
| `chats.empty.desc` | "Generate an invite link to host a connection, or paste an invite from a peer to join." |
| `chats.empty.action` | "Get Started" |
| `chats.search.empty` | "No conversations found" |
| `chats.search.hint` | "Try adjusting your search terms or clear the filter." |
| `chats.preview.empty` | "No messages yet." |
| `chats.archived` | "Archived" |
| `chats.actions.favorite` | "Add to favorites" |
| `chats.actions.unfavorite` | "Remove from favorites" |
| `chats.actions.archive` | "Archive conversation" |
| `chats.actions.unarchive` | "Unarchive" |

### 29.6 Messaging Strings

| Key | String |
|-----|--------|
| `chat.header.encrypted` | "Encrypted Session" |
| `chat.input.placeholder` | "Type a secure message…" |
| `chat.input.attach` | "Attach a file" |
| `chat.input.emoji` | "Add emoji" |
| `chat.input.timer` | "Self-destruct timer" |
| `chat.input.send` | "Send message (Ctrl+Enter)" |
| `chat.timer.off` | "Off" |
| `chat.timer.5s` | "5s" |
| `chat.timer.30s` | "30s" |
| `chat.timer.1m` | "1m" |
| `chat.timer.5m` | "5m" |
| `chat.timer.1h` | "1h" |
| `chat.timer.24h` | "24h" |
| `chat.footer.encrypted` | "End-to-end encrypted" |
| `chat.footer.shortcuts` | "Ctrl+Enter to send · Esc to go back" |
| `chat.empty.title` | "Start the conversation" |
| `chat.empty.desc` | "Send a message below to begin your encrypted conversation." |
| `chat.typing` | "Peer is typing…" |
| `chat.search.placeholder` | "Search messages… (press Esc to close)" |
| `chat.status.sending` | "Sending…" |
| `chat.status.sent` | "Sent" |
| `chat.status.delivered` | "Delivered" |
| `chat.status.read` | "Read" |
| `chat.edited` | "edited" |
| `chat.self_destruct` | "Self-destructs in {m}:{s}" |
| `chat.deleted` | "Message deleted" |
| `chat.loading_older` | "Loading older messages…" |
| `chat.beginning` | "Beginning of conversation" |
| `chat.dropzone.hint` | "Drop files here to send" |
| `chat.context.edit` | "Edit" |
| `chat.context.delete` | "Delete" |

### 29.7 File Transfer Strings

| Key | String |
|-----|--------|
| `file.request.accept` | "Accept" |
| `file.request.reject` | "Reject" |
| `file.progress.sending` | "sending" |
| `file.progress.transferring` | "transferring" |
| `file.progress.cancelled` | "cancelled" |
| `file.progress.completed` | "completed" |
| `file.progress.failed` | "failed" |
| `file.remaining` | "{s}s remaining" |

### 29.8 Security Strings

| Key | String |
|-----|--------|
| `security.verify.title` | "Verify Peer Fingerprint" |
| `security.verify.desc` | "Compare fingerprints via a secure out-of-band channel (e.g., in-person, phone call, or another encrypted app)." |
| `security.verify.local` | "You (Local)" |
| `security.verify.peer` | "Peer" |
| `security.verify.matched` | "Matched" |
| `security.verify.unverified` | "Not yet verified" |
| `security.verify.confirm` | "Confirm Match & Verify" |
| `security.verify.success` | "Peer verified" |
| `security.verify.fail` | "Fingerprints do not match. Do NOT proceed." |

### 29.9 Settings Strings

| Key | String |
|-----|--------|
| `settings.title` | "Settings" |
| `settings.section.identity` | "Identity" |
| `settings.section.theme` | "Theme" |
| `settings.section.network` | "Network" |
| `settings.section.discovery` | "Discovery" |
| `settings.section.security` | "Security" |
| `settings.section.stun` | "STUN Servers" |
| `settings.section.about` | "About" |
| `settings.label.fingerprint` | "Fingerprint" |
| `settings.label.public_key` | "Public Key" |
| `settings.label.public_ip` | "Public IP" |
| `settings.label.nat_type` | "NAT Type" |
| `settings.label.stun_servers` | "STUN Servers" |
| `settings.label.private_mode` | "Private Mode" |
| `settings.label.tor` | "Tor" |
| `settings.label.connectivity` | "Connectivity" |
| `settings.label.appearance` | "Appearance" |
| `settings.label.accent_color` | "Accent Color" |
| `settings.label.screen_capture` | "Screen Capture Protection" |
| `settings.label.clipboard_clear` | "Clipboard Auto-Clear" |
| `settings.label.idle_lock` | "Idle Vault Lock" |
| `settings.label.vault` | "Vault" |
| `settings.label.version` | "Version" |
| `settings.label.crypto` | "Crypto" |

### 29.10 Error Strings (Summary)

| Key | String |
|-----|--------|
| `error.unknown` | "An unexpected error occurred." |
| `error.ipc` | "Internal error. Please restart the application." |
| `error.database` | "Database error. Some features may not be available." |
| `error.restart` | "Please restart M2M for this change to take effect." |
| `error.retry` | "Please try again." |
| `error.offline` | "You are offline. Some features are unavailable." |

---

## 30. Testing Matrix

### 30.1 Component Test Requirements

| Component | Unit Tests | Interaction Tests | Accessibility Tests | Visual Regression |
|-----------|-----------|-------------------|-------------------|-------------------|
| Button | 5 variants × 6 states = 30 | Click, hover, focus, disabled | aria-label, focus ring | All variants |
| Input | 3 variants × 5 states = 15 | Typing, focus, clear, paste | aria-invalid, aria-describedby | All variants |
| Card | 2 variants × 3 states = 6 | Click (if clickable) | role="region" | All variants |
| Modal | 1 × 4 states = 4 | Open, close, escape, backdrop | Focus trap, aria-modal | Open/close anim |
| Badge | 5 variants × 2 sizes = 10 | N/A | aria-label | All variants |
| Toast | 4 types = 4 | Auto-dismiss, hover pause | role="alert" | All types |
| Select | 1 × 3 states = 3 | Open, select, keyboard | aria-label | Open state |
| ConversationItem | 1 × 4 states = 4 | Click, hover, actions | aria-label, role | All states |
| MessageBubble | 2 directions × 3 states = 6 | Hover reaction picker, context menu | role, aria-label | Sent/received |
| EmojiPicker | 1 × 2 states = 2 | Select emoji, close, keyboard nav | aria-label, grid role | Open state |
| TypingIndicator | 1 × 2 states = 2 | Appear, disappear | aria-live | Both states |

### 30.2 Flow Test Requirements

| Flow | Integration Tests | E2E Tests |
|------|------------------|-----------|
| First launch → Setup → Onboarding → Vault → Hub | 5 | 1 |
| Return user → Vault unlock → Hub → Chat | 3 | 1 |
| Generate invite → Copy → Connect | 3 | 1 |
| Send message → Receive → React → Edit → Delete | 5 | 1 |
| File transfer (send → progress → complete) | 4 | 1 |
| File transfer (send → cancel) | 2 | 1 |
| Reconnect (disconnect → reconnect → success) | 3 | 1 |
| Reconnect (disconnect → reconnect → fail) | 3 | 1 |
| Theme change (light → dark → system) | 3 | 1 |
| Accent color change → persist → restore | 3 | 1 |
| Favorites toggle → sort → persist | 3 | 1 |
| Archive toggle → hide → unarchive | 3 | 1 |
| Contact search → result → open | 2 | 1 |
| Message search → result → scroll | 2 | 1 |
| Settings STUN add → remove → reset | 3 | 1 |
| Settings Tor toggle → test | 2 | 1 |
| Vault lock → idle auto-lock | 2 | 1 |
| Group create → invite → message | 4 | 1 |
| Error boundary → recover | 2 | 1 |

---

## 31. Timer & Timeout Specifications

### 31.1 Timer Registry

| Timer | Duration | Element | Action |
|-------|----------|---------|--------|
| Typing indicator idle | 3s | ChatView | Send TypingIndicatorClear after last keystroke |
| Typing indicator hide | 3s | ChatView | Hide typing banner after last packet |
| Toast auto-dismiss (success) | 4s | Toast | Remove toast |
| Toast auto-dismiss (info) | 5s | Toast | Remove toast |
| Toast auto-dismiss (warning) | 6s | Toast | Remove toast |
| Toast auto-dismiss (error) | 8s | Toast | Remove toast |
| Copy feedback | 2s | Any copy button | Reset checkmark to original icon |
| Connection timeout | 15s | TCP connect | Fail connection attempt |
| Handshake timeout | 30s | X3DH | Fail handshake |
| Chunk ACK timeout | 5s | File transfer | Retry chunk (max 3 retries) |
| Heartbeat interval | 30s | Connection | Send Heartbeat packet |
| Heartbeat timeout | 10s | Connection | Mark connection dead if no HeartbeatAck |
| Invite default validity | 60min | Invite | Expire invite |
| Invite max validity | 24h | Invite | Hard cap on invite lifetime |
| Self-destruct timer minimum | 5s | Message | Minimum timer value |
| Self-destruct timer maximum | 24h | Message | Maximum timer value |
| Reconnect backoff base | 1s | Reconnect | First attempt delay |
| Reconnect backoff max | 30s | Reconnect | Cap on exponential backoff |
| Reconnect max attempts | 5 | Reconnect | Maximum retries |
| Offline queue flush | On reconnect | Queue | Send all queued messages |
| Missed message sync | On reconnect | Sync | Request missed messages |
| Message edit window | 24h | Message | Max time to allow edits |
| Clipboard clear | Configurable (5s-60s) | Security | Auto-clear clipboard |
| Idle lock | Configurable (1m-30m) | Security | Auto-lock vault |
| Self-destruct cleanup | 10s interval | Timer | Remove expired messages |
| STUN discovery | 5s max | Network | Parallel STUN query timeout |
| View transition | 500ms | Navigation | Animation duration |
| Onboarding auto-advance | 3s | Setup | Auto-advance from loading |

---

## 32. Focus Trap & Keyboard Edge Cases

### 32.1 Focus Trap Specification

**Modal focus trap**:
```
When modal opens:
  1. Save currently focused element reference
  2. Set focus to first focusable element inside modal
  3. On Tab: cycle forward through modal elements
  4. On Shift+Tab: cycle backward through modal elements
  5. On Escape: close modal, return focus to saved element
  6. On click outside: close modal, return focus

Focusable elements within modal:
  - All buttons
  - All inputs
  - All selects
  - All textareas
  - All links
  - Elements with tabIndex >= 0

Elements NOT focusable within modal:
  - Disabled elements
  - Elements with tabIndex = -1
  - Non-interactive elements (text, icons, etc.)
```

**Context menu focus trap**:
```
When context menu opens:
  1. Focus first menu item
  2. Arrow Up/Down: navigate items
  3. Enter/Space: activate item, close menu
  4. Escape: close menu, return focus to message
  5. Click outside: close menu
```

### 32.2 Keyboard Edge Cases

| Scenario | Expected Behavior |
|----------|------------------|
| Modal open → Tab key pressed | Focus cycles within modal, never reaches background |
| Modal open → Shift+Tab from first element | Focus wraps to last element in modal |
| Modal open → background element clicked | Modal closes, focus returns to trigger |
| Two modals stacked | Only top modal is interactive. Close top modal to access lower one |
| Toast appears during modal open | Toast is still visible (higher z-index) but not focusable |
| Context menu open → resize window | Menu repositions or closes (prefer close) |
| Input focused → Escape key | If input has value: clear input. If empty: blur input |
| Input focused → modal opens | Input blurs, modal receives focus |
| Tab through disabled button | Disabled elements are skipped in tab order |
| Ctrl+Tab in modal | Should NOT switch browser tabs (prevent default) |
| Alt+F4 (Windows) | Closes window to tray |
| Cmd+Q (macOS) | Quits application |
| Cmd+W (macOS) / Ctrl+W (Windows) | Hides to tray |
| Session locked → keyboard shortcut | Ignored — vault must be unlocked first |
| Multiple monitors | App remembers last position on reopen |

---

*Part 3 covers every error message (100+ catalogued), every tooltip, every loading skeleton with visual ASCII specs, the complete database schema (every table/column/index across 3 databases), the z-index map, offline/degraded behavior, error boundary specs, the state sync protocol with event registry, the full string catalog (~200 user-facing strings), the testing matrix, every timer/timeout specification, and keyboard/focus-trap edge cases. This completes the Apple/Linear/Notion-grade product specification.*
