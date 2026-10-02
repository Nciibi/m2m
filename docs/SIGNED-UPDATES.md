# M2M Signed Updates (Tauri Updater)

The unsigned update channel is the kill-chain (roadmap §4): a compromised
download server or MITM delivers malware that runs with full messenger
trust. Tauri's updater verifies an Ed25519 signature over every update
artifact BEFORE install; without the private key, updates cannot be forged.

## Current state

- `tauri-plugin-updater` is registered in `src-tauri/src/lib.rs`.
- **`bundle.createUpdaterArtifacts` is `false` and must stay that way until a
  maintainer completes the setup below.** It used to be `true` next to a
  placeholder `pubkey`, and that combination makes `tauri build` finish writing
  its `.deb` and `.rpm` and *then exit non-zero* with "A public key has been
  found, but no private key". That silently broke every packaging path at once:
  `.github/workflows/ci.yml` uploaded nothing (`if-no-files-found: ignore`
  swallowed it) and `release.yml` could not publish. An unsigned build is the
  honest default; a build that exits non-zero while claiming success is not.
- **The channel is INERT until configured**: `plugins.updater.endpoints` is
  empty and the pubkey is the literal placeholder `REPLACE_WITH_RELEASE_PUBKEY`.
  No update check can succeed (or run) until a maintainer fills these in.
- A third gate: `src-tauri/capabilities/default.json` grants **no `updater:*`
  permission**, so even with a valid pubkey and endpoint the frontend cannot
  invoke the updater. Add the permission as part of step 2 below — the original
  three steps were not sufficient on their own.

## One-time maintainer setup

1. Generate the release keypair (keep the PRIVATE key offline, e.g.
   encrypted USB / hardware-backed secret store):
   ```
   npx @tauri-apps/cli signer generate -w ~/.tauri/m2m.key
   ```
2. Put the PUBLIC key from that command into
   `src-tauri/tauri.conf.json -> plugins.updater.pubkey`, and grant the
   updater capability in `src-tauri/capabilities/default.json`:
   ```json
   "permissions": ["core:default", "updater:default"]
   ```
3. Set `bundle.createUpdaterArtifacts` to `true`. Confirm `tauri build` now
   exits 0 with `TAURI_SIGNING_PRIVATE_KEY` exported.
4. Host a `latest.json` manifest at your update endpoint and list it in
   `plugins.updater.endpoints` (HTTPS only), format:
   ```json
   {
     "version": "5.0.0",
     "notes": "...",
     "platforms": {
       "windows-x86_64": { "signature": "<contents of .sig file>", "url": "https://.../M2M_5.0.0_x64.msi.zip" }
     }
   }
   ```
5. At release time, sign artifacts with the private key:
   ```
   export TAURI_SIGNING_PRIVATE_KEY=$(cat ~/.tauri/m2m.key)
   npx @tauri-apps/cli build
   ```
   The `.sig` files emitted next to each artifact are what goes into
   `latest.json`. Note `.github/workflows/release.yml` currently passes the
   private key but also sets `includeUpdaterJson: false`, which generates a
   signature and then discards it — fix that in the same change that turns
   signing on.

## Rules

- NEVER commit `m2m.key`. Losing it means rotating keys AND shipping a
  trust-on-first-use migration — treat it like root access to every install.
- NEVER commit a placeholder or throwaway `pubkey` with
  `createUpdaterArtifacts: true`. That exact pairing is what made the release
  pipeline fail while appearing to work.
- Reproducible builds (`scripts/build-release.sh`) + published SHA-256
  hashes let third parties verify source↔binary equivalence independently
  of the updater signature. That claim is still **unverified**: the script has
  not been run twice on two machines.
