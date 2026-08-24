# M2M Hardened VM Image

Debian-slim image with **Tor + M2M preconfigured** (roadmap §4).

## Build & run

```sh
# 1. Produce a reproducible release binary (see scripts/build-release.sh)
../scripts/build-release.sh /tmp/release

# 2. Verify the binary's SHA-256 against the published hashes
sha256sum /tmp/release/m2m

# 3. Build the image
cp /tmp/release/m2m .
docker build -t m2m-hardened -f Dockerfile .

# 4. Run (GUI: mount your Wayland/X11 socket)
docker run --rm -it \
    -e XDG_RUNTIME_DIR=/tmp \
    -v ${XDG_RUNTIME_DIR}/${WAYLAND_DISPLAY:-wayland-0}:/tmp/${WAYLAND_DISPLAY:-wayland-0} \
    -v m2m-data:/home/m2muser/.m2m \
    m2m-hardened
```

## Threat model — what this DOES and DOES NOT give you

| Provides | Does NOT provide |
|---|---|
| Minimal attack surface (no compilers/shells in image) | Hypervisor/container-escape protection |
| Tor available for M2M private mode | Automatic Tor routing of peer TCP connections (M2M connects direct; enable private mode + SOCKS5 in-app) |
| Ephemeral by default unless `-v m2m-data` mounted | Protection against a compromised host |
| Reproducible-ish from a pinned Debian base + verified binary | Kernel-level keylogger defense |

Pair with in-app settings: Air-Gap Mode, Ephemeral Conversations,
Screen-Capture Protection, Duress Passphrase.

## Honest limits

- The GUI requires a display socket; headless hosts can still use M2M as a
  LAN listener node.
- `torrc` is client-only (`ClientOnly 1`) — this image must never become a
  relay.
