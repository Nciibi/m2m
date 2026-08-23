# Running M2M in High-Risk Environments

Operational guide for Tails, Whonix, and Qubes OS. Complements the honest
threat model in `SECURITY-HARDENING.md` §2: an app cannot defeat kernel-
level malware on the same machine — true isolation requires replacing the
host environment. These setups do exactly that.

## Universal app settings (all environments)

Enable these **before** first contact:

| Setting | Why |
|---|---|
| Screen Capture Protection | Hides the window from recorders/shares |
| Blur When Unfocused | Defeats background capture |
| Capture Software Detection | Warns while OBS/snipping tools run |
| Air-Gap Mode | Hard-blocks STUN/UPnP/relay/Tor toggles — LAN only |
| Ephemeral Conversations | Nothing touches disk |
| Send Batching + Typing Cover Traffic | Timing-metadata resistance |
| Idle Vault Lock + Clipboard Auto-Clear | Unattended-access hygiene |

## Tails (amnesic live system)

Tails routes EVERYTHING through Tor and wipes state on reboot.

1. Boot Tails → set administration password → connect to Tor.
2. M2M's own listener is TCP-direct; on Tails, direct inbound connections
   break Tor isolation expectations. Run M2M **LAN-only**:
   enable **Air-Gap Mode** and connect only to peers on your local network.
3. Persistence: without enabling Tails Persistent Storage, keys/vault die
   with the session — pair well with **Ephemeral Conversations** for a
   leave-no-trace workflow. If you DO add persistence, note it lives
   outside M2M's own encryption boundary; full-disk assumptions change.
4. Do NOT disable the Tor firewall. M2M's STUN is blocked by air-gap mode;
   direct connects only to LAN addresses.

**Honest limits**: Tails' Tor routing does NOT proxy M2M's raw TCP peer
connections (M2M is not SOCKS-integrated end-to-end). Treat Tails+M2M as
"amnesic LAN messenger", not "anonymous internet messenger".

## Whonix (Tor gateway + workstation VMs)

1. Run M2M inside the Whonix **Workstation**; all traffic is forced through
   the Gateway by firewall rules — an app cannot leak around it.
2. Same settings table as above; **Air-Gap Mode ON** unless you have
   specifically reasoned about which internet operations you want.
3. Inbound connections: Whonix blocks unsolicited inbound by default.
   For LAN-style use between two workstations on the same host, use host-
   internal networking (`Whonix-Workstation` ↔ isolated VM) rather than
   exposing ports.
4. Snapshot the Workstation BEFORE first unlock: reverting after a session
   gives you amnesia like Tails, but with persistence when you choose it.

## Qubes OS (compartmentalization)

The strongest option: M2M gets its OWN qube.

1. Create a disposable or standalone qube (e.g. `m2m-qube`, Debian-based).
2. Networking:
   - Maximum isolation: `sys-firewall` (no Tor) or an offline qube for
     pure LAN use + **Air-Gap Mode**.
   - Tor-forced: attach through `sys-whonix`.
3. Never reuse the M2M qube for browsing/email. One qube, one purpose.
4. Use Qubes' clipboard policies: copy/paste into the m2m-qube only via
   explicit global clipboard operations (Ctrl+Shift+C / Ctrl+Shift+V) —
   this pairs with M2M's own clipboard auto-clear.
5. DispVM workflow for maximally sensitive sessions: start M2M in a
   Disposable VM, everything (including vault) evaporates at shutdown —
   combine with **Ephemeral Conversations** so nothing persists even if
   the template caches something unexpected.

## What NONE of these can fix

- A compromised hypervisor/host (Qubes threat model explicitly excludes it).
- Hardware keyloggers, HDMI capture, phone pointed at the screen.
- Coerced passphrase disclosure → register a duress passphrase (Settings ▸
  Security) if this is a realistic risk in your situation.
