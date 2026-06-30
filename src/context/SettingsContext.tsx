import {
  createContext, useContext, useState, useCallback, useRef, ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { useApp } from "./AppContext";
import type { NetworkSettings, StunConfig, NatTypeInfo, DiscoveryConfig, DiscoveredPeer, SecurityConfig } from "../types";

interface SettingsContextValue {
  networkSettings: NetworkSettings | null;
  publicIp: string | null;
  stunLoading: boolean;
  networkDiagnostics: NatTypeInfo | null;
  stunConfig: StunConfig | null;
  stunServerInput: string;
  setStunServerInput: (v: string) => void;
  privateMode: boolean;
  connectivityResult: any;
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
  handleConnectDiscoveredPeer: (address: string) => Promise<void>;
  handleRefreshDiscovery: () => Promise<void>;
  // Security
  securityConfig: SecurityConfig | null;
  handleScreenCaptureToggle: () => Promise<void>;
  handleClipboardClearSecsChange: (secs: number) => Promise<void>;
  handleIdleLockSecsChange: (secs: number) => Promise<void>;
  handleLockVault: () => Promise<void>;
  handleClearClipboard: () => Promise<void>;
}

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function useSettings(): SettingsContextValue {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error("useSettings() must be used within <SettingsProvider>");
  return ctx;
}

export function SettingsProvider({ children }: { children: ReactNode }) {
  const { addToast, setView } = useApp();

  const [networkSettings, setNetworkSettings] = useState<NetworkSettings | null>(null);
  const [publicIp, setPublicIp] = useState<string | null>(null);
  const [stunLoading, setStunLoading] = useState(false);
  const [networkDiagnostics, setNetworkDiagnostics] = useState<NatTypeInfo | null>(null);
  const [stunConfig, setStunConfig] = useState<StunConfig | null>(null);
  const [stunServerInput, setStunServerInput] = useState("");
  const [privateMode, setPrivateMode] = useState(false);
  const [connectivityResult, setConnectivityResult] = useState<any>(null);
  // Discovery state
  const [discoveryConfig, setDiscoveryConfig] = useState<DiscoveryConfig | null>(null);
  const [discoveredPeers, setDiscoveredPeers] = useState<DiscoveredPeer[]>([]);
  // Security state
  const [securityConfig, setSecurityConfig] = useState<SecurityConfig | null>(null);
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
      try { setDiscoveredPeers(await invoke<DiscoveredPeer[]>("get_discovered_peers")); }
      catch { /* noop */ }
    } catch { /* noop */ }
  }, [setView]);

  const handleStunDiscover = useCallback(async () => {
    setStunLoading(true);
    try {
      setPublicIp(await invoke<string>("discover_public_ip"));
      setNetworkDiagnostics(await invoke<NatTypeInfo>("get_network_diagnostics"));
    } catch (e) {
      addToast("STUN failed: " + e, "error");
    } finally {
      setStunLoading(false);
    }
  }, [addToast]);

  const handleAddStunServer = useCallback(async () => {
    if (!stunConfig || !stunServerInput.trim()) return;
    const newServers = [...stunConfig.servers, stunServerInput.trim()];
    try {
      await invoke("set_stun_servers", { servers: newServers });
      setStunConfig({ ...stunConfig, servers: newServers });
      setStunServerInput("");
    } catch (e) {
      addToast("Failed to add STUN server: " + e, "error");
    }
  }, [stunConfig, stunServerInput, addToast]);

  const handleRemoveStunServer = useCallback(async (idx: number) => {
    if (!stunConfig) return;
    const newServers = stunConfig.servers.filter((_, i) => i !== idx);
    if (newServers.length === 0) {
      addToast("Cannot remove all STUN servers — at least one required.", "warning");
      return;
    }
    try {
      await invoke("set_stun_servers", { servers: newServers });
      setStunConfig({ ...stunConfig, servers: newServers });
    } catch (e) {
      addToast("Failed to remove STUN server: " + e, "error");
    }
  }, [stunConfig, addToast]);

  const handleResetStunDefaults = useCallback(async () => {
    const defaults = ["stun.l.google.com:19302", "stun1.l.google.com:19302", "stun.cloudflare.com:3478", "stun.nextcloud.com:3478"];
    try {
      await invoke("set_stun_servers", { servers: defaults });
      setStunConfig(stunConfig ? { ...stunConfig, servers: defaults } : null);
    } catch (e) {
      addToast("Failed to reset STUN servers: " + e, "error");
    }
  }, [stunConfig, addToast]);

  const handlePrivateModeToggle = useCallback(async () => {
    const newVal = !privateMode;
    try {
      await invoke("set_private_mode", { enabled: newVal });
      setPrivateMode(newVal);
    } catch { /* noop */ }
  }, [privateMode]);

  const handleConnectivityCheck = useCallback(async () => {
    try {
      setConnectivityResult(await invoke<any>("check_connectivity"));
      setNetworkDiagnostics(await invoke<NatTypeInfo>("get_network_diagnostics"));
    } catch (e) {
      addToast("Connectivity check failed: " + e, "error");
    }
  }, [addToast]);

  const handleTorToggle = useCallback(async () => {
    if (!networkSettings) return;
    const newVal = !networkSettings.tor_enabled;
    try {
      await invoke("set_tor_enabled", { enabled: newVal });
      setNetworkSettings({ ...networkSettings, tor_enabled: newVal });
    } catch (e) {
      addToast("Tor toggle failed: " + e, "error");
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
      addToast("LAN discovery toggle failed: " + e, "error");
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
      addToast("DHT discovery toggle failed: " + e, "error");
    }
  }, [discoveryConfig, addToast]);

  const handleConnectDiscoveredPeer = useCallback(async (address: string) => {
    try {
      const info = await invoke<any>("connect_discovered_peer", { address });
      addToast("Connected to discovered peer", "success");
      return info;
    } catch (e) {
      addToast("Connection to discovered peer failed: " + e, "error");
      throw e;
    }
  }, [addToast]);

  const handleRefreshDiscovery = useCallback(async () => {
    try {
      const peers = await invoke<DiscoveredPeer[]>("refresh_discovery");
      setDiscoveredPeers(peers);
    } catch (e) {
      addToast("Refresh discovery failed: " + e, "error");
    }
  }, [addToast]);

  return (
    <SettingsContext.Provider value={{
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
    }}>
      {children}
    </SettingsContext.Provider>
  );
}
