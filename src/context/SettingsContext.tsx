import {
  createContext, useContext, useState, useCallback, useEffect, useMemo, useRef, ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { useApp } from "./AppContext";
import { errorMessage, MIN_STUN_SERVERS, MAX_STUN_SERVERS } from "../utils";
import { useT } from "../i18n/I18nContext";
import type {
  CaptureCapability,
  ConnectionInfo,
  ConnectivityStatus,
  DiscoveredPeer,
  DiscoveryConfig,
  NatTypeInfo,
  NetworkSettings,
  SecurityConfig,
  StorageUsage,
  StunConfig,
  RelayConfigView,
} from "../types";

interface SettingsContextValue {
  networkSettings: NetworkSettings | null;
  publicIp: string | null;
  stunLoading: boolean;
  networkDiagnostics: NatTypeInfo | null;
  stunConfig: StunConfig | null;
  stunServerInput: string;
  setStunServerInput: (v: string) => void;
  privateMode: boolean;
  connectivityResult: ConnectivityStatus | null;
  openSettings: () => Promise<void>;
  handleStunDiscover: () => Promise<void>;
  handleAddStunServer: () => Promise<void>;
  handleRemoveStunServer: (idx: number) => Promise<void>;
  handleResetStunDefaults: () => Promise<void>;
  handlePrivateModeToggle: () => Promise<void>;
  handleConnectivityCheck: () => Promise<void>;
  handleTorToggle: () => Promise<void>;
  // Discovery
  discoveryConfig: DiscoveryConfig | null;
  discoveredPeers: DiscoveredPeer[];
  handleLanToggle: () => Promise<void>;
  handleDhtToggle: () => Promise<void>;
  /** Resolves with the established connection; rethrows on failure. */
  handleConnectDiscoveredPeer: (address: string) => Promise<ConnectionInfo>;
  handleRefreshDiscovery: () => Promise<void>;
  // Relay. `RelayConfigView` is what the backend returns from `get_relay_config`
  // — host and port plus a *boolean* for the token. The token itself never
  // crosses the IPC boundary, so it cannot be re-displayed or logged here, and
  // the form therefore tracks its own token draft separately.
  relayConfig: RelayConfigView | null;
  /** Live form state for the host field, so typing does not round-trip IPC. */
  relayHost: string;
  /** Live form state for the port field. Kept as a string so a partially-typed
   *  value is representable; validated as a number on save. */
  relayPort: string;
  setRelayHost: (host: string) => void;
  setRelayPort: (port: string) => void;
  /** Empty clears the relay, which is what disabling it means. */
  handleRelaySave: () => Promise<void>;
  handleRelayClear: () => Promise<void>;
  relaySaving: boolean;
  // Security
  securityConfig: SecurityConfig | null;
  captureCapability: CaptureCapability | null;
  handleScreenCaptureToggle: () => Promise<void>;
  handleCaptureDetectionToggle: () => Promise<void>;
  handleBlurOnFocusLossToggle: () => Promise<void>;
  handleAirGapToggle: () => Promise<void>;
  handleEphemeralModeToggle: () => Promise<void>;
  handleSendBatchingChange: (ms: number) => Promise<void>;
  handleCoverTypingToggle: () => Promise<void>;
  handlePanicHotkeyArmToggle: () => Promise<void>;
  duressConfigured: boolean;
  setDuressPassphrase: (passphrase: string) => Promise<void>;
  clearDuressPassphrase: () => Promise<void>;
  refreshDuressStatus: () => Promise<void>;
  handleClipboardClearSecsChange: (secs: number) => Promise<void>;
  /**
   * Raise or lower the ceiling on stored message history.
   *
   * Persisted via the shared `DEFAULT_SECURITY_CONFIG` so the new field is
   * never dropped by a handler that spread a stale local default.
   */
  handleStorageCapChange: (bytes: number) => Promise<void>;
  /** Current usage against the cap, or null until it has been read. */
  storageUsage: StorageUsage | null;
  refreshStorageUsage: () => Promise<void>;
  handleIdleLockSecsChange: (secs: number) => Promise<void>;
  handleRequireKnownContactToggle: () => Promise<void>;
  handleLockVault: () => Promise<void>;
  handleClearClipboard: () => Promise<void>;
  scheduleClipboardClear: (secs: number) => void;
}

/**
 * The single source of truth for "no config loaded yet".
 *
 * This was four hand-written copies of the same object literal, one per toggle
 * handler. Adding a field to `SecurityConfig` and updating three of the four
 * would have compiled cleanly and left one toggle sending a config that
 * silently reset the new field on save — the same hand-synchronised-copy
 * failure as the duplicated `AAD_MSG_STORE` constant.
 *
 * `storage_cap_bytes` is 0 here, which `effective_storage_cap()` on the Rust
 * side resolves to the 10 GiB default rather than to "no limit". A 0 in the
 * default is deliberate: it is the value `SecurityConfig::default()` produces,
 * so the two sides agree.
 */
export const DEFAULT_SECURITY_CONFIG: SecurityConfig = {
  screen_capture_protection: false,
  clipboard_clear_secs: 0,
  idle_lock_secs: 0,
  require_known_contact: false,
  capture_process_detection: false,
  blur_on_focus_loss: false,
  air_gap_mode: false,
  ephemeral_mode: false,
  send_batching_ms: 0,
  cover_typing_traffic: false,
  panic_hotkey_enabled: false,
  storage_cap_bytes: 0,
};

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function useSettings(): SettingsContextValue {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error("useSettings() must be used within <SettingsProvider>");
  return ctx;
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  const { addToast, setView } = useApp();
  const t = useT();

  const [networkSettings, setNetworkSettings] = useState<NetworkSettings | null>(null);
  const [publicIp, setPublicIp] = useState<string | null>(null);
  const [stunLoading, setStunLoading] = useState(false);
  const [networkDiagnostics, setNetworkDiagnostics] = useState<NatTypeInfo | null>(null);
  const [stunConfig, setStunConfig] = useState<StunConfig | null>(null);
  const [stunServerInput, setStunServerInput] = useState("");
  const [privateMode, setPrivateMode] = useState(false);
  const [connectivityResult, setConnectivityResult] = useState<ConnectivityStatus | null>(null);
  // Discovery state
  const [discoveryConfig, setDiscoveryConfig] = useState<DiscoveryConfig | null>(null);
  const [discoveredPeers, setDiscoveredPeers] = useState<DiscoveredPeer[]>([]);
  // Security state
  const [securityConfig, setSecurityConfig] = useState<SecurityConfig | null>(null);
  const [captureCapability, setCaptureCapability] = useState<CaptureCapability | null>(null);
  const [duressConfigured, setDuressConfigured] = useState(false);
  // Clipboard clear timer ref
  const clipboardTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const openSettings = useCallback(async () => {
    setView("settings");
    try {
      const ns = await invoke<NetworkSettings>("get_network_settings");
      setNetworkSettings(ns);
      setPublicIp(ns.public_ip);
      const sc = await invoke<StunConfig>("get_stun_config");
      setStunConfig(sc);
      setPrivateMode(sc.private_mode);
      try { setNetworkDiagnostics(await invoke<NatTypeInfo>("get_network_diagnostics")); }
      catch { /* noop */ }
      try { setDiscoveryConfig(await invoke<DiscoveryConfig>("get_discovery_config")); }
      catch { /* noop */ }
      // `discoveredPeers` is rendered as `.length` and mapped over, so a
      // non-array result crashes the view. Validate every read that feeds a
      // shape assumption rather than trusting the backend.
      try {
        const peers = await invoke<DiscoveredPeer[] | null>("get_discovered_peers");
        setDiscoveredPeers(Array.isArray(peers) ? peers : []);
      }
      catch { /* noop */ }
      try { setSecurityConfig(await invoke<SecurityConfig>("get_security_config")); }
      catch { /* noop */ }
      try { setCaptureCapability(await invoke<CaptureCapability>("get_capture_capability")); }
      catch { /* noop */ }
      try { setDuressConfigured(await invoke<boolean>("is_duress_configured")); }
      catch { /* noop */ }
    } catch { /* noop */ }
  }, [setView]);

  /**
   * Load the persisted security config on mount.
   *
   * This is a security control, not a UI convenience, so it cannot depend on
   * the user happening to open Settings. It used to be fetched only inside
   * `openSettings`, which meant that on a fresh launch `securityConfig` was
   * `null` and the three headline protections were all inert:
   *
   *   - the panic-wipe hotkey was never bound (its keydown listener is only
   *     attached when `panic_hotkey_enabled` is true)
   *   - the idle auto-lock timeout fell back to 0 (disabled)
   *   - focus-loss blur fell back to false
   *
   * A user who armed panic wipe, quit, and then needed it in an emergency would
   * have pressed the hotkey and had *nothing happen*. The backend protections
   * did survive the restart (App.tsx calls `reapply_security_config`), which
   * made it worse: the setting renders as enabled in Settings while the
   * frontend behaviour is dead.
   *
   * `App.tsx` is at 0% test coverage and the Settings view tests mock
   * `useSettings` wholesale, which is why this survived so long.
   */
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const sc = await invoke<SecurityConfig>("get_security_config");
        if (!cancelled) setSecurityConfig(sc);
      } catch (e) {
        // Leave it null; consumers already fail closed (`?? false` / `?? 0`).
        // Surface it, though — a user who armed protections deserves to know
        // they are not currently active.
        if (!cancelled) {
          console.error("failed to load security config", e);
          addToast("Could not load security settings — protections may be inactive", "error");
        }
      }
    })();
    return () => { cancelled = true; };
  }, [addToast]);

  const handleStunDiscover = useCallback(async () => {
    setStunLoading(true);
    try {
      setPublicIp(await invoke<string>("discover_public_ip"));
      setNetworkDiagnostics(await invoke<NatTypeInfo>("get_network_diagnostics"));
    } catch (e) {
      addToast("STUN failed: " + errorMessage(e), "error");
    } finally {
      setStunLoading(false);
    }
  }, [addToast]);

  const handleAddStunServer = useCallback(async () => {
    if (!stunConfig || !stunServerInput.trim()) return;
    const newServers = [...stunConfig.servers, stunServerInput.trim()];
    // Pre-empt the backend's bounds rather than letting the round-trip fail.
    //
    // Without the floor this is an unrecoverable dead end below two entries:
    // adding one to an empty list produces a 1-entry list, the backend rejects
    // it, `setStunConfig` never runs, so the list stays empty and the next add
    // produces 1 again. Only "Reset to defaults" escaped.
    if (newServers.length < MIN_STUN_SERVERS) {
      addToast(
        `Add ${MIN_STUN_SERVERS - newServers.length} more STUN server(s) before saving — ` +
        "agreement between independent servers is what makes the published address trustworthy.",
        "warning",
      );
      return;
    }
    if (newServers.length > MAX_STUN_SERVERS) {
      addToast(`At most ${MAX_STUN_SERVERS} STUN servers.`, "warning");
      return;
    }
    try {
      await invoke("set_stun_servers", { servers: newServers });
      setStunConfig({ ...stunConfig, servers: newServers });
      setStunServerInput("");
    } catch (e) {
      addToast("Failed to add STUN server: " + errorMessage(e), "error");
    }
  }, [stunConfig, stunServerInput, addToast]);

  const handleRemoveStunServer = useCallback(async (idx: number) => {
    if (!stunConfig) return;
    const newServers = stunConfig.servers.filter((_, i) => i !== idx);
    // Two, not one. `consensus` used to mean "every server that *responded*
    // agreed", which is vacuously true when exactly one answers — so a single
    // rogue server, or one DNS-hijacked hostname, defined the address this app
    // published. The backend now requires a quorum, so refuse here rather than
    // letting the round-trip fail with a raw backend error.
    if (newServers.length < MIN_STUN_SERVERS) {
      addToast(
        `Cannot go below ${MIN_STUN_SERVERS} STUN servers — agreement between ` +
        "independent servers is what makes the published address trustworthy.",
        "warning",
      );
      return;
    }
    try {
      await invoke("set_stun_servers", { servers: newServers });
      setStunConfig({ ...stunConfig, servers: newServers });
    } catch (e) {
      addToast("Failed to remove STUN server: " + errorMessage(e), "error");
    }
  }, [stunConfig, addToast]);

  const handleResetStunDefaults = useCallback(async () => {
    const defaults = ["stun.l.google.com:19302", "stun1.l.google.com:19302", "stun.cloudflare.com:3478", "stun.nextcloud.com:3478"];
    try {
      await invoke("set_stun_servers", { servers: defaults });
      setStunConfig(stunConfig ? { ...stunConfig, servers: defaults } : null);
    } catch (e) {
      addToast("Failed to reset STUN servers: " + errorMessage(e), "error");
    }
  }, [stunConfig, addToast]);

  const handlePrivateModeToggle = useCallback(async () => {
    const newVal = !privateMode;
    try {
      await invoke("set_private_mode", { enabled: newVal });
      setPrivateMode(newVal);
    } catch (e) {
      // Private mode is what keeps the real IP out of invites. Silently
      // failing to enable it would leave the user believing they are protected
      // while their address is broadcast.
      addToast("Failed to " + (newVal ? "enable" : "disable") + " private mode: " + errorMessage(e), "error");
    }
  }, [privateMode, addToast]);

  const handleConnectivityCheck = useCallback(async () => {
    try {
      setConnectivityResult(await invoke<ConnectivityStatus>("check_connectivity"));
      setNetworkDiagnostics(await invoke<NatTypeInfo>("get_network_diagnostics"));
    } catch (e) {
      addToast("Connectivity check failed: " + errorMessage(e), "error");
    }
  }, [addToast]);

const handleTorToggle = useCallback(async () => {
    // Refuse rather than silently no-op. This control is what the entire
    // `dial.rs` chokepoint depends on, and the previous bare `return` left
    // the caller free to flip its own optimistic state - so a Tor checkbox
    // could read "enabled" with nothing persisted anywhere.
    if (!networkSettings) {
      addToast("Network settings not loaded yet - try again in a moment", "error");
      return;
    }
    const newVal = !networkSettings.tor_enabled;
    try {
      await invoke("set_tor_enabled", { enabled: newVal });
      setNetworkSettings({ ...networkSettings, tor_enabled: newVal });
    } catch (e) {
      // Left `networkSettings` untouched, so the checkbox cannot show a state
      // the backend rejected.
      addToast("Tor toggle failed: " + errorMessage(e), "error");
    }
  }, [networkSettings, addToast]);

  // ── Discovery handlers ──

  const handleLanToggle = useCallback(async () => {
    const current = discoveryConfig ?? { lan_enabled: false, dht_enabled: false };
    const newConfig: DiscoveryConfig = {
      ...current,
      lan_enabled: !current.lan_enabled,
    };
    try {
      const result = await invoke<DiscoveryConfig>("set_discovery_config", { config: newConfig });
      setDiscoveryConfig(result);
      const peers = await invoke<DiscoveredPeer[]>("get_discovered_peers");
      setDiscoveredPeers(peers);
    } catch (e) {
      addToast("LAN discovery toggle failed: " + errorMessage(e), "error");
    }
  }, [discoveryConfig, addToast]);

  const handleDhtToggle = useCallback(async () => {
    const current = discoveryConfig ?? { lan_enabled: false, dht_enabled: false };
    const newConfig: DiscoveryConfig = {
      ...current,
      dht_enabled: !current.dht_enabled,
    };
    try {
      const result = await invoke<DiscoveryConfig>("set_discovery_config", { config: newConfig });
      setDiscoveryConfig(result);
      const peers = await invoke<DiscoveredPeer[]>("get_discovered_peers");
      setDiscoveredPeers(peers);
    } catch (e) {
      addToast("DHT discovery toggle failed: " + errorMessage(e), "error");
    }
  }, [discoveryConfig, addToast]);

  const handleConnectDiscoveredPeer = useCallback(async (address: string) => {
    try {
      // Returns `commands::ConnectionInfo`; typed here because the value is
      // used to synthesise a conversation entry downstream.
      const info = await invoke<ConnectionInfo>("connect_discovered_peer", { address });
      addToast("Connected to discovered peer", "success");
      return info;
    } catch (e) {
      addToast("Connection to discovered peer failed: " + errorMessage(e), "error");
      throw e;
    }
  }, [addToast]);

  const handleRefreshDiscovery = useCallback(async () => {
    try {
      const peers = await invoke<DiscoveredPeer[] | null>("refresh_discovery");
      // Guard the shape: this value is rendered as `discoveredPeers.length`
      // and iterated, so a null/undefined from the backend crashes the view on
      // the next render. Validate rather than trust.
      setDiscoveredPeers(Array.isArray(peers) ? peers : []);
    } catch (e) {
      addToast("Refresh discovery failed: " + errorMessage(e), "error");
    }
  }, [addToast]);

  // -- Relay --
  //
  // `get_relay_config` / `set_relay_config` existed and were registered with
  // Tauri, but nothing in `src/` ever called them: a user had no way to point
  // M2M at a relay at all, while the relay server shipped in the same repo.
  // These handlers are that missing wiring.
  //
  // Host and port are local form state so typing does not round-trip IPC per
  // keystroke. The token is tracked in the component that owns the field and
  // never here, because the backend will not give it back.
  const [relayConfig, setRelayConfig] = useState<RelayConfigView | null>(null);
  const [relayHost, setRelayHost] = useState("");
  const [relayPort, setRelayPort] = useState("");
  const [relaySaving, setRelaySaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const cfg = await invoke<RelayConfigView | null>("get_relay_config");
        if (cancelled) return;
        setRelayConfig(cfg);
        // Only seed the inputs when a relay exists. Seeding from null would
        // blank whatever the user has typed on an unrelated re-render.
        if (cfg) {
          setRelayHost(cfg.host);
          setRelayPort(String(cfg.port));
        }
      } catch (e) {
        if (!cancelled) addToast("Could not read relay config: " + errorMessage(e), "error");
      }
    })();
    return () => { cancelled = true; };
  }, [addToast]);

  const handleRelaySave = useCallback(async () => {
    const host = relayHost.trim();
    if (!host) {
      addToast("Relay host cannot be empty. Use Clear to disable the relay.", "error");
      return;
    }
    const port = Number.parseInt(relayPort, 10);
    // Validate before the IPC: `parseInt("80abc")` is 80 and `parseInt("")` is
    // NaN, so a typo would otherwise be silently accepted.
    if (!Number.isInteger(port) || port <= 0 || port > 65535) {
      addToast("Relay port must be a whole number between 1 and 65535.", "error");
      return;
    }
    setRelaySaving(true);
    try {
      await invoke("set_relay_config", {
        config: { host, port, auth_token: "" },
      });
      // Re-read rather than assume: what is rendered must be what was stored.
      const cfg = await invoke<RelayConfigView | null>("get_relay_config");
      setRelayConfig(cfg);
      addToast(`Relay set to ${host}:${port}.`, "success");
    } catch (e) {
      addToast("Failed to set relay: " + errorMessage(e), "error");
    } finally {
      setRelaySaving(false);
    }
  }, [relayHost, relayPort, addToast]);

  const handleRelayClear = useCallback(async () => {
    setRelaySaving(true);
    try {
      await invoke("set_relay_config", { config: null });
      setRelayConfig(null);
      setRelayHost("");
      setRelayPort("");
      addToast("Relay disabled.", "success");
    } catch (e) {
      addToast("Failed to disable relay: " + errorMessage(e), "error");
    } finally {
      setRelaySaving(false);
    }
  }, [addToast]);
// ── Clipboard auto-clear helper ──

  // Clear the pending clipboard timer on unmount, otherwise it fires
  // `setState` after the provider is gone and a manual dismiss is followed by
  // a redundant backend call.
  useEffect(() => () => {
    if (clipboardTimerRef.current) {
      clearTimeout(clipboardTimerRef.current);
      clipboardTimerRef.current = null;
    }
  }, []);

const scheduleClipboardClear = useCallback((secs: number) => {
    if (clipboardTimerRef.current) {
      clearTimeout(clipboardTimerRef.current);
    }
    // Arm the Rust-side deadline FIRST, so the guarantee exists even if this
    // webview is throttled, backgrounded or dead before `secs` elapses.
    //
    // `SecurityConfig::clipboard_clear_secs` had no reader anywhere in Rust: the
    // whole feature was this `setTimeout`. A timer that lives in the component
    // being protected stops running exactly when the app is not being used, and
    // a copied passphrase sitting in the OS clipboard is the exact thing a
    // "clipboard auto-clear: 30s" setting promises will not happen.
    //
    // A failure here is not fatal — the local timer below still runs — so it is
    // logged rather than surfaced, which would be a false alarm.
    invoke("arm_clipboard_auto_clear", { secs })
      .catch((e) => console.warn("could not arm Rust clipboard deadline:", e));
    if (secs > 0) {
      clipboardTimerRef.current = setTimeout(async () => {
        try {
          await invoke("clear_clipboard");
          // Also clear via web API as fallback
          try { await navigator.clipboard.writeText(""); } catch { /* noop */ }
        } catch {
          // The user was told "clipboard auto-clear: 30s". If the clear fails,
          // a copied passphrase or fingerprint sits in the clipboard
          // indefinitely with no feedback at all. That is the worst possible
          // failure for this specific feature, so it is surfaced loudly.
          addToast("Clipboard auto-clear FAILED - clear it manually", "error");
        }
      }, secs * 1000);
    }
    // `addToast` is not guaranteed referentially stable, and a stale copy here
    // would surface the failure through a detached toast list.
  }, [addToast]);

  // ── Security handlers ──

  const handleScreenCaptureToggle = useCallback(async () => {
    const current = securityConfig ?? DEFAULT_SECURITY_CONFIG;
    const newConfig: SecurityConfig = {
      ...current,
      screen_capture_protection: !current.screen_capture_protection,
    };
    try {
      const result = await invoke<SecurityConfig>("set_security_config", { config: newConfig });
      setSecurityConfig(result);
      addToast(
        result.screen_capture_protection ? "Screen capture protection enabled" : "Screen capture protection disabled",
        "info",
      );
    } catch (e) {
      addToast("Failed to toggle screen capture protection: " + errorMessage(e), "error");
    }
  }, [securityConfig, addToast]);

  const handleCaptureDetectionToggle = useCallback(async () => {
    const current = securityConfig ?? DEFAULT_SECURITY_CONFIG;
    const newConfig: SecurityConfig = { ...current, capture_process_detection: !current.capture_process_detection };
    try {
      const result = await invoke<SecurityConfig>("set_security_config", { config: newConfig });
      setSecurityConfig(result);
      addToast(
        result.capture_process_detection
          ? "Capture software detection enabled"
          : "Capture software detection disabled",
        "info",
      );
    } catch (e) {
      addToast("Failed to toggle capture detection: " + errorMessage(e), "error");
    }
  }, [securityConfig, addToast]);

  const handleBlurOnFocusLossToggle = useCallback(async () => {
    const current = securityConfig ?? DEFAULT_SECURITY_CONFIG;
    const newConfig: SecurityConfig = { ...current, blur_on_focus_loss: !current.blur_on_focus_loss };
    try {
      const result = await invoke<SecurityConfig>("set_security_config", { config: newConfig });
      setSecurityConfig(result);
    } catch (e) {
      addToast("Failed to toggle focus blur: " + errorMessage(e), "error");
    }
  }, [securityConfig, addToast]);

  const handleClipboardClearSecsChange = useCallback(async (secs: number) => {
    const current = securityConfig ?? DEFAULT_SECURITY_CONFIG;
    const newConfig: SecurityConfig = { ...current, clipboard_clear_secs: secs };
    try {
      const result = await invoke<SecurityConfig>("set_security_config", { config: newConfig });
      setSecurityConfig(result);
    } catch (e) {
      addToast("Failed to update clipboard setting: " + errorMessage(e), "error");
    }
  }, [securityConfig, addToast]);

  const [storageUsage, setStorageUsage] = useState<StorageUsage | null>(null);

  const refreshStorageUsage = useCallback(async () => {
    try {
      setStorageUsage(await invoke<StorageUsage>("get_storage_usage"));
    } catch (e) {
      // Non-fatal: the settings row shows "Loading…" and the cap can still be
      // changed. A failed read must not block the control.
      console.warn("get_storage_usage failed", e);
    }
  }, []);

  const handleStorageCapChange = useCallback(
    async (bytes: number) => {
      const current = securityConfig ?? DEFAULT_SECURITY_CONFIG;
      const newConfig: SecurityConfig = { ...current, storage_cap_bytes: bytes };
      try {
        const result = await invoke<SecurityConfig>("set_security_config", { config: newConfig });
        setSecurityConfig(result);
        // Re-read so the "x of y used" line reflects the new cap immediately
        // rather than waiting for a remount.
        await refreshStorageUsage();
        addToast("Storage cap updated", "success");
      } catch (e) {
        addToast("Failed to update storage cap: " + errorMessage(e), "error");
      }
    },
    [securityConfig, addToast, refreshStorageUsage],
  );

  const handleIdleLockSecsChange = useCallback(async (secs: number) => {
    const current = securityConfig ?? DEFAULT_SECURITY_CONFIG;
    const newConfig: SecurityConfig = { ...current, idle_lock_secs: secs };
    try {
      const result = await invoke<SecurityConfig>("set_security_config", { config: newConfig });
      setSecurityConfig(result);
    } catch (e) {
      addToast("Failed to update idle lock setting: " + errorMessage(e), "error");
    }
  }, [securityConfig, addToast]);

  const handleRequireKnownContactToggle = useCallback(async () => {
    const current = securityConfig ?? DEFAULT_SECURITY_CONFIG;
    const newConfig: SecurityConfig = {
      ...current,
      require_known_contact: !current.require_known_contact,
    };
    try {
      const result = await invoke<SecurityConfig>("set_security_config", { config: newConfig });
      setSecurityConfig(result);
      addToast(
        result.require_known_contact
          ? "Known contacts only — strangers can no longer connect"
          : "Known contacts only disabled — anyone may connect",
        "info",
      );
    } catch (e) {
      addToast("Failed to toggle known contacts only: " + errorMessage(e), "error");
    }
  }, [securityConfig, addToast]);

  const handleAirGapToggle = useCallback(async () => {
    const current = securityConfig ?? DEFAULT_SECURITY_CONFIG;
    const newConfig: SecurityConfig = { ...current, air_gap_mode: !current.air_gap_mode };
    try {
      const result = await invoke<SecurityConfig>("set_security_config", { config: newConfig });
      setSecurityConfig(result);
      addToast(
        result.air_gap_mode
          ? "Air-gap mode ON — internet-facing operations blocked (LAN only)"
          : "Air-gap mode off — internet operations allowed again",
        result.air_gap_mode ? "warning" : "info",
      );
    } catch (e) {
      addToast("Failed to toggle air-gap mode: " + errorMessage(e), "error");
    }
  }, [securityConfig, addToast]);

  const handleEphemeralModeToggle = useCallback(async () => {
    const current = securityConfig ?? DEFAULT_SECURITY_CONFIG;
    const newConfig: SecurityConfig = { ...current, ephemeral_mode: !current.ephemeral_mode };
    try {
      const result = await invoke<SecurityConfig>("set_security_config", { config: newConfig });
      setSecurityConfig(result);
      addToast(
        result.ephemeral_mode
          ? "Ephemeral mode ON — conversations stay in RAM only"
          : "Ephemeral mode off — conversations persist to encrypted storage",
        "info",
      );
    } catch (e) {
      addToast("Failed to toggle ephemeral mode: " + errorMessage(e), "error");
    }
  }, [securityConfig, addToast]);

  const handleSendBatchingChange = useCallback(async (ms: number) => {
    const current = securityConfig ?? DEFAULT_SECURITY_CONFIG;
    const newConfig: SecurityConfig = { ...current, send_batching_ms: ms };
    try {
      const result = await invoke<SecurityConfig>("set_security_config", { config: newConfig });
      setSecurityConfig(result);
    } catch (e) {
      addToast("Failed to update send batching: " + errorMessage(e), "error");
    }
  }, [securityConfig, addToast]);

  const handleCoverTypingToggle = useCallback(async () => {
    const current = securityConfig ?? DEFAULT_SECURITY_CONFIG;
    const newConfig: SecurityConfig = { ...current, cover_typing_traffic: !current.cover_typing_traffic };
    try {
      const result = await invoke<SecurityConfig>("set_security_config", { config: newConfig });
      setSecurityConfig(result);
    } catch (e) {
      addToast("Failed to toggle typing cover traffic: " + errorMessage(e), "error");
    }
  }, [securityConfig, addToast]);

  // ── Duress passphrase ──

  const setDuressPassphrase = useCallback(async (passphrase: string) => {
    try {
      await invoke("set_duress_passphrase", { passphrase });
      setDuressConfigured(true);
      addToast("Duress passphrase registered — entering it at unlock will WIPE the vault", "warning");
    } catch (e) {
      addToast("Failed to register duress passphrase: " + errorMessage(e), "error");
      throw e;
    }
  }, [addToast]);

  const clearDuressPassphrase = useCallback(async () => {
    try {
      await invoke("clear_duress_passphrase");
      setDuressConfigured(false);
      addToast("Duress passphrase removed", "info");
    } catch (e) {
      addToast("Failed to remove duress passphrase: " + errorMessage(e), "error");
      throw e;
    }
  }, [addToast]);

  const refreshDuressStatus = useCallback(async () => {
    try { setDuressConfigured(await invoke<boolean>("is_duress_configured")); }
    catch { /* noop */ }
  }, []);


  const handlePanicHotkeyArmToggle = useCallback(async () => {
    const current = securityConfig ?? DEFAULT_SECURITY_CONFIG;
    const arming = !current.panic_hotkey_enabled;
    const newConfig: SecurityConfig = { ...current, panic_hotkey_enabled: arming };
    try {
      const result = await invoke<SecurityConfig>("set_security_config", { config: newConfig });
      setSecurityConfig(result);
      addToast(
        arming ? t("toast.panicArmed") : t("toast.panicDisarmed"),
        arming ? "warning" : "info",
      );
    } catch (e) {
      addToast(t("toast.panicToggleFailed", { err: errorMessage(e) }), "error");
    }
  }, [securityConfig, addToast, t]);

  /**
   * Arm or disarm the panic hotkey.
   *
   * Arming is a destructive, irreversible action, so the View must confirm it
   * in a real dialog first and then call this. Disarming needs no prompt, so
   * this is the single place the config is actually written.
   */
  const setPanicHotkeyArmed = handlePanicHotkeyArmToggle;

  const handleLockVault = useCallback(async () => {
    try {
      await invoke("lock_vault");
      addToast("Vault locked", "success");
    } catch (e) {
      addToast("Failed to lock vault: " + errorMessage(e), "error");
    }
  }, [addToast]);

  const handleClearClipboard = useCallback(async () => {
    try {
      await invoke("clear_clipboard");
      try { await navigator.clipboard.writeText(""); } catch { /* noop */ }
      addToast("Clipboard cleared", "info");
    } catch (e) {
      addToast("Failed to clear clipboard: " + errorMessage(e), "error");
    }
  }, [addToast]);

  /**
   * Memoized so consumers only re-render when the context actually changes.
   *
   * The individual handlers were all `useCallback`'d, but the wrapping object
   * literal was constructed fresh on every render, which defeats every one of
   * them: any `setState` here re-rendered every `useSettings()` consumer, and
   * `SettingsView` pulls the whole context, so toggling one checkbox re-rendered
   * the entire settings page. The callbacks being stable was worth nothing.
   */
  const value = useMemo<SettingsContextValue>(() => ({
    networkSettings, publicIp, stunLoading, networkDiagnostics,
    stunConfig, stunServerInput, setStunServerInput,
    privateMode, connectivityResult,
    openSettings,
    handleStunDiscover, handleAddStunServer, handleRemoveStunServer,
    handleResetStunDefaults, handlePrivateModeToggle,
    handleConnectivityCheck, handleTorToggle,
    discoveryConfig, discoveredPeers,
    handleLanToggle, handleDhtToggle,
    handleConnectDiscoveredPeer, handleRefreshDiscovery,
    relayConfig, relayHost, relayPort, setRelayHost, setRelayPort,
    handleRelaySave, handleRelayClear, relaySaving,
    securityConfig,
    captureCapability,
    handleScreenCaptureToggle, handleCaptureDetectionToggle, handleBlurOnFocusLossToggle,
    handleAirGapToggle, handleEphemeralModeToggle, handleSendBatchingChange, handleCoverTypingToggle,
    handlePanicHotkeyArmToggle: setPanicHotkeyArmed,
    duressConfigured, setDuressPassphrase, clearDuressPassphrase, refreshDuressStatus,
    handleClipboardClearSecsChange,
    handleStorageCapChange, storageUsage, refreshStorageUsage,
    handleIdleLockSecsChange, handleRequireKnownContactToggle, handleLockVault, handleClearClipboard,
    scheduleClipboardClear,
  }), [
    networkSettings, publicIp, stunLoading, networkDiagnostics,
    stunConfig, stunServerInput, privateMode, connectivityResult,
    openSettings,
    handleStunDiscover, handleAddStunServer, handleRemoveStunServer,
    handleResetStunDefaults, handlePrivateModeToggle,
    handleConnectivityCheck, handleTorToggle,
    discoveryConfig, discoveredPeers,
    handleLanToggle, handleDhtToggle,
    handleConnectDiscoveredPeer, handleRefreshDiscovery,
    relayConfig, setRelayHost, setRelayPort,
    handleRelaySave, handleRelayClear, relaySaving,
    securityConfig, captureCapability,
    handleScreenCaptureToggle, handleCaptureDetectionToggle, handleBlurOnFocusLossToggle,
    handleAirGapToggle, handleEphemeralModeToggle, handleSendBatchingChange,
    handleCoverTypingToggle,
    duressConfigured, setDuressPassphrase, clearDuressPassphrase, refreshDuressStatus,
    handleClipboardClearSecsChange,
    handleStorageCapChange, storageUsage, refreshStorageUsage,
    handleIdleLockSecsChange, handleRequireKnownContactToggle, handleLockVault,
    handleClearClipboard, scheduleClipboardClear, setPanicHotkeyArmed,
  ]);

  return (
    <SettingsContext.Provider value={value}>
      {children}
    </SettingsContext.Provider>
  );
}
