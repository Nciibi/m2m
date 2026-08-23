# M2M Signed Updates (Tauri Updater)

The unsigned update channel is the kill-chain (roadmap §4): a compromised
download server or MITM delivers malware that runs with full messenger
trust. Tauri's updater verifies an Ed25519 signature over every update
artifact BEFORE install; without the private key, updates cannot be forged.

## Current state

- `tauri-plugin-updater` is registered in `src-tauri/src/lib.rs`.
- `tauri.conf.json` has `bundle.createUpdaterArtifacts: true`, so every
  release bundle produces `.sig` signatures alongside artifacts.
- **The channel is INERT until configured**: `plugins.updater.endpoints`
  is empty and the pubkey is a placeholder. No update check can succeed
  (or run) until a maintainer fills these in.

## One-time maintainer setup

1. Generate the release keypair (keep the PRIVATE key offline, e.g.
   encrypted USB / hardware-backed secret store):
   ```
   npx @tauri-apps/cli signer generate -w ~/.tauri/m2m.key
   ```
2. Put the PUBLIC key from that command into
   `src-tauri/tauri.conf.json → plugins.updater.pubkey`.
3. Host a `latest.json` manifest at your update endpoint and list it in
   `plugins.updater.endpoints` (HTTPS only), format:
   ```json
   {
     "version": "3.7.0",
     "notes": "...",
     "platforms": {
       "windows-x86_64": { "signature": "<contents of .sig file>", "url": "https://.../M2M_3.7.0_x64.msi.zip" }
     }
   }
   ```
4. At release time, sign artifacts with the private key:
   ```
   export TAURI_SIGNING_PRIVATE_KEY=$(cat ~/.tauri/m2m.key)
   npx @tauri-apps/cli build
   ```
   The `.sig` files emitted next to each artifact are what goes into
   `latest.json`.

## Rules

- NEVER commit `m2m.key`. Losing it means rotating keys AND shipping a
  trust-on-first-use migration — treat it like root access to every install.
- Reproducible builds (`scripts/build-release.sh`) + published SHA-256
  hashes let third parties verify source↔binary equivalence independently
  of the updater signature.
