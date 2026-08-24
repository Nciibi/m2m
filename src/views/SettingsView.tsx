import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { Button, Input, Badge, ToastContainer } from "../components/ui";
import { ArrowLeftIcon, GearIcon, CopyIcon, CheckIcon, CloseIcon, WifiIcon, GlobeIcon, LockIcon, EyeOffIcon, MonitorIcon, SunIcon, MoonIcon } from "../components/ui/Icons";
import Sidebar from "../components/Sidebar";
import { useApp } from "../context/AppContext";
import { useSettings } from "../context/SettingsContext";
import { useTheme } from "../context/ThemeContext";

export default function SettingsView() {
  const { identity, toasts, addToast, removeToast, setView } = useApp();
  const { theme, setTheme, resolvedTheme, accentColor, setAccentColor } = useTheme();
  const {
    networkSettings, publicIp, stunLoading, networkDiagnostics,
    stunConfig, stunServerInput, privateMode, connectivityResult,
    handleStunDiscover, handleAddStunServer,
    handleRemoveStunServer, handleResetStunDefaults, handlePrivateModeToggle,
    handleConnectivityCheck, handleTorToggle, setStunServerInput,
    discoveryConfig, discoveredPeers,
    handleLanToggle, handleDhtToggle, handleRefreshDiscovery,
    securityConfig,
    captureCapability,
    handleScreenCaptureToggle, handleCaptureDetectionToggle, handleBlurOnFocusLossToggle,
    handleAirGapToggle, handleEphemeralModeToggle, handleSendBatchingChange, handleCoverTypingToggle,
    handlePanicHotkeyArmToggle,
    duressConfigured, setDuressPassphrase, clearDuressPassphrase,
    handleClipboardClearSecsChange,
    handleIdleLockSecsChange, handleRequireKnownContactToggle, handleLockVault, handleClearClipboard,
    scheduleClipboardClear,
  } = useSettings();
  const [fpCopied, setFpCopied] = useState(false);
  const [ipCopied, setIpCopied] = useState(false);
  const [torEnabled, setTorEnabled] = useState(networkSettings?.tor_enabled ?? false);
  const [appVersion, setAppVersion] = useState<string>("");

  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => setAppVersion(""));
  }, []);

  const onBackToHub = () => setView("hub");

  return (
    <div className="app-shell">
      <Sidebar currentView="settings" onNavigate={setView} />
      <div className="app-main">
      <div className="app-header">
        <h1 className="app-header__title">
          <span className="app-header__icon-bg app-header__icon-bg--accent">
            <GearIcon size={18} color="white" />
          </span>
          Settings
        </h1>
        <div className="app-header__actions">
          <Button variant="secondary" size="sm" onClick={onBackToHub}><ArrowLeftIcon size={16} /> Hub</Button>
        </div>
      </div>

      <div className="app-content settings-content">
        {/* ─── Identity ─── */}
        <section className="settings-section">
          <h2 className="settings-section__title">Identity</h2>
          <div className="settings-card">
            <div className="settings-row">
              <span className="settings-label">Fingerprint</span>
              <span className="settings-mono">{identity?.fingerprint || "—"}</span>
              <button className="btn btn--ghost btn--icon-sm" onClick={() => {
                if (identity?.fingerprint) {
                  navigator.clipboard.writeText(identity.fingerprint);
                  setFpCopied(true);
                  setTimeout(() => setFpCopied(false), 2000);
                  if (securityConfig?.clipboard_clear_secs && securityConfig.clipboard_clear_secs > 0) {
                    scheduleClipboardClear(securityConfig.clipboard_clear_secs);
                  }
                }
              }} aria-label="Copy fingerprint">
                {fpCopied ? <CheckIcon size={14} /> : <CopyIcon size={14} />}
              </button>
            </div>
            <div className="settings-row">
              <span className="settings-label">Public Key</span>
              <span className="settings-mono settings-mono--truncate">{identity?.public_key_hex || "—"}</span>
            </div>
            <div className="settings-row">
              <Button
                variant="danger"
                onClick={async () => {
                  try {
                    await invoke("lock_vault");
                    setView("vault");
                  } catch (e) {
                    addToast("Failed to sign out: " + e, "error");
                  }
                }}
              >
                Sign Out
              </Button>
              <span className="text-muted text-sm">Locks the vault and returns to the unlock screen.</span>
            </div>
          </div>
        </section>

        {/* ─── Network ─── */}
        <section className="settings-section">
          <h2 className="settings-section__title">Network</h2>
          <div className="settings-card">
            <div className="settings-row">
              <span className="settings-label">Public IP</span>
              <span className="settings-mono">{publicIp || "Not yet discovered"}</span>
              {publicIp && (
                <button className="btn btn--ghost btn--icon-sm" onClick={() => {
                  navigator.clipboard.writeText(publicIp);
                  setIpCopied(true);
                  setTimeout(() => setIpCopied(false), 2000);
                  if (securityConfig?.clipboard_clear_secs && securityConfig.clipboard_clear_secs > 0) {
                    scheduleClipboardClear(securityConfig.clipboard_clear_secs);
                  }
                }} aria-label="Copy IP">
                  {ipCopied ? <CheckIcon size={14} /> : <CopyIcon size={14} />}
                </button>
              )}
              <Button size="xs" onClick={handleStunDiscover} loading={stunLoading}>Discover via STUN</Button>
            </div>

            {networkDiagnostics && (
              <>
                <div className="settings-row">
                  <span className="settings-label">NAT Type</span>
                  <Badge variant={(["FullCone", "RestrictedCone", "PortRestrictedCone"]).includes(networkDiagnostics.nat_type) ? "success" : "warning"}>
                    {networkDiagnostics.nat_type}
                  </Badge>
                </div>
                <div className="settings-row">
                  <span className="settings-label">STUN Servers</span>
                  <span>{networkDiagnostics.stun_servers?.filter(s => s.reachable).length ?? 0}/{networkDiagnostics.stun_servers?.length ?? 0} reachable</span>
                </div>
              </>
            )}

            <div className="settings-divider" />

            <div className="settings-row">
              <span className="settings-label">Private Mode</span>
              <label className="toggle">
                <input type="checkbox" checked={privateMode} onChange={handlePrivateModeToggle} aria-label="Toggle private mode" />
                <span className="toggle-slider" />
              </label>
              <span className="settings-hint">Hide IP from invites</span>
            </div>

            <div className="settings-row">
              <span className="settings-label">Tor</span>
              <label className="toggle">
                <input type="checkbox" checked={torEnabled} onChange={async () => { await handleTorToggle(); setTorEnabled(!torEnabled); }} aria-label="Toggle Tor" />
                <span className="toggle-slider" />
              </label>
              <span className="settings-hint">Route connections via Tor</span>
              <Button size="xs" variant="secondary" onClick={async () => {
                addToast("Testing Tor…", "info");
                try {
                  const result = await invoke<any>("check_connectivity");
                  const torOk = result?.tor_reachable ?? result?.tor ?? false;
                  addToast(torOk ? "Tor ✓" : "Tor not reachable via current proxy", torOk ? "success" : "warning");
                } catch (e) {
                  addToast("Tor test unavailable: " + e, "warning");
                }
              }}>Test Tor</Button>
            </div>

            <div className="settings-divider" />

            <div className="settings-row">
              <span className="settings-label">Connectivity</span>
              <Button size="xs" onClick={handleConnectivityCheck}>Check</Button>
            </div>
            {connectivityResult && (
              <div className="settings-row">
                <span className="settings-label">Result</span>
                <span className="settings-mono">{JSON.stringify(connectivityResult)}</span>
              </div>
            )}
          </div>
        </section>

        {/* ─── Discovery ─── */}
        <section className="settings-section">
          <h2 className="settings-section__title">Discovery</h2>
          <div className="settings-card">
            <div className="settings-row">
              <span className="settings-label"><WifiIcon size={16} /> LAN Discovery</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={discoveryConfig?.lan_enabled ?? false}
                  onChange={handleLanToggle}
                  aria-label="Toggle LAN discovery"
                />
                <span className="toggle-slider" />
              </label>
              <span className="settings-hint">Broadcast presence on local WiFi</span>
            </div>

            <div className="settings-row">
              <span className="settings-label"><GlobeIcon size={16} /> DHT Discovery</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={discoveryConfig?.dht_enabled ?? false}
                  onChange={handleDhtToggle}
                  aria-label="Toggle DHT discovery"
                />
                <span className="toggle-slider" />
              </label>
              <span className="settings-hint">Discover peers via DHT network</span>
            </div>

            <div className="settings-row">
              <span className="settings-label">Discovered Peers</span>
              <span>{discoveredPeers.length} found</span>
              <Button size="xs" variant="secondary" onClick={handleRefreshDiscovery}>Refresh</Button>
            </div>

            <div className="settings-divider" />

            <p className="text-muted text-sm">
              ⚠️ Both are <strong>OFF by default</strong> for privacy. When enabled,
              your IP address is visible to observers on the discovery channel.
              Ephemeral IDs are used (not your permanent identity key) and
              rotate periodically.
            </p>
          </div>
        </section>

        {/* ─── Security ─── */}
        <section className="settings-section">
          <h2 className="settings-section__title">Security</h2>
          <div className="settings-card">
            <div className="settings-row">
              <span className="settings-label"><EyeOffIcon size={16} /> Screen Capture Protection</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={securityConfig?.screen_capture_protection ?? false}
                  onChange={handleScreenCaptureToggle}
                  aria-label="Toggle screen capture protection"
                />
                <span className="toggle-slider" />
              </label>
              <span className="settings-hint">Prevent window from appearing in screenshots</span>
            </div>
            {captureCapability && (
              <div className="settings-row">
                <span
                  className="settings-hint"
                  style={{
                    color: captureCapability.level === "unsupported" ? "var(--color-danger)" : "var(--color-text-muted)",
                    maxWidth: "100%",
                  }}
                  role="note"
                >
                  {captureCapability.level === "full" ? "✔ Full protection on this platform — " : captureCapability.level === "partial" ? "△ Partial protection on this platform — " : "✖ Not effective on this platform — "}
                  {captureCapability.note}
                </span>
              </div>
            )}

            <div className="settings-row">
              <span className="settings-label">Capture Software Detection</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={securityConfig?.capture_process_detection ?? false}
                  onChange={handleCaptureDetectionToggle}
                  aria-label="Toggle capture software detection"
                />
                <span className="toggle-slider" />
              </label>
              <span className="settings-hint">Warn while OBS, Snipping Tool, and other recorders are running (stops nothing — detection only)</span>
            </div>

            <div className="settings-row">
              <span className="settings-label">Blur When Unfocused</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={securityConfig?.blur_on_focus_loss ?? false}
                  onChange={handleBlurOnFocusLossToggle}
                  aria-label="Toggle blur when window loses focus"
                />
                <span className="toggle-slider" />
              </label>
              <span className="settings-hint">Blur all content whenever the window loses focus or is hidden</span>
            </div>

            <div className="settings-row">
              <span className="settings-label"><LockIcon size={16} /> Air-Gap Mode</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={securityConfig?.air_gap_mode ?? false}
                  onChange={handleAirGapToggle}
                  aria-label="Toggle air-gap mode"
                />
                <span className="toggle-slider" />
              </label>
              <span className="settings-hint">LAN-only: blocks STUN, port forwarding, relay registration, discovery, and Tor</span>
            </div>

            <div className="settings-row">
              <span className="settings-label"><LockIcon size={16} /> Ephemeral Conversations</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={securityConfig?.ephemeral_mode ?? false}
                  onChange={handleEphemeralModeToggle}
                  aria-label="Toggle ephemeral conversations"
                />
                <span className="toggle-slider" />
              </label>
              <span className="settings-hint">RAM only: no message, reaction, or edit is ever written to disk</span>
            </div>

            <div className="settings-row">
              <span className="settings-label">Send Batching Delay</span>
              <select className="select--compact"
                value={securityConfig?.send_batching_ms ?? 0}
                onChange={e => handleSendBatchingChange(parseInt(e.target.value, 10))}
                aria-label="Random send delay for traffic analysis resistance"
              >
                <option value={0}>Off</option>
                <option value={250}>~0–250ms</option>
                <option value={1000}>~0–1s</option>
                <option value={5000}>~0–5s</option>
              </select>
              <span className="settings-hint">Random pre-send delay so message timing leaks less</span>
            </div>

            <div className="settings-row">
              <span className="settings-label">Typing Cover Traffic</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={securityConfig?.cover_typing_traffic ?? false}
                  onChange={handleCoverTypingToggle}
                  aria-label="Toggle typing indicator cover traffic"
                />
                <span className="toggle-slider" />
              </label>
              <span className="settings-hint">Randomize typing-indicator timing so keystroke cadence leaks less</span>
            </div>

            <div className="settings-row">
              <span className="settings-label">Duress Passphrase</span>
              {duressConfigured ? (
                <Button variant="secondary" size="xs" onClick={() => clearDuressPassphrase()}>Remove</Button>
              ) : (
                <Button variant="secondary" size="xs" onClick={async () => {
                  const input = window.prompt(
                    "Set a DISTINCT duress passphrase (min 12 chars).\n\n⚠ IRREVERSIBLE: entering it at unlock will silently DELETE all local data and show a normal wrong-password error.\n\nThere is no confirmation at unlock — that is the point.",
                    ""
                  );
                  if (!input) return;
                  if (!window.confirm("Register this duress passphrase? Entering it at unlock wipes the vault. This cannot be undone.")) return;
                  await setDuressPassphrase(input);
                }}>Set…</Button>
              )}
              <span className="settings-hint">{duressConfigured ? "Registered — entering it at unlock wipes the vault" : "Coercion resistance: a special passphrase that wipes everything"}</span>
            </div>

            <div className="settings-row">
              <span className="settings-label"><LockIcon size={16} /> Panic Hotkey (Ctrl+Alt+Shift+W)</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={securityConfig?.panic_hotkey_enabled ?? false}
                  onChange={handlePanicHotkeyArmToggle}
                  aria-label="Arm emergency panic wipe hotkey"
                />
                <span className="toggle-slider" />
              </label>
              <span className="settings-hint">{securityConfig?.panic_hotkey_enabled ? "ARMED — hotkey deletes everything instantly, no confirmation" : "Emergency wipe: delete all data and exit instantly"}</span>
            </div>

            <div className="settings-row">
              <span className="settings-label">Clipboard Auto-Clear</span>
              <select className="select--compact"
                value={securityConfig?.clipboard_clear_secs ?? 0}
                onChange={e => handleClipboardClearSecsChange(parseInt(e.target.value, 10))}
                aria-label="Clipboard auto-clear timeout"
              >
                <option value={0}>Off</option>
                <option value={5}>5s</option>
                <option value={10}>10s</option>
                <option value={30}>30s</option>
                <option value={60}>1m</option>
              </select>
              <span className="settings-hint">Auto-clear clipboard after copying sensitive data</span>
            </div>

            <div className="settings-row">
              <span className="settings-label">Idle Vault Lock</span>
              <select className="select--compact"
                value={securityConfig?.idle_lock_secs ?? 0}
                onChange={e => handleIdleLockSecsChange(parseInt(e.target.value, 10))}
                aria-label="Idle vault lock timeout"
              >
                <option value={0}>Off</option>
                <option value={60}>1m</option>
                <option value={300}>5m</option>
                <option value={600}>10m</option>
                <option value={1800}>30m</option>
              </select>
              <span className="settings-hint">Auto-lock vault after inactivity</span>
            </div>

            <div className="settings-row">
              <span className="settings-label"><LockIcon size={16} /> Known Contacts Only</span>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={securityConfig?.require_known_contact ?? false}
                  onChange={handleRequireKnownContactToggle}
                  aria-label="Toggle known contacts only"
                />
                <span className="toggle-slider" />
              </label>
              <span className="settings-hint">Reject incoming connections from strangers (first-time invites require turning this off)</span>
            </div>

            <div className="settings-divider" />

            <div className="settings-row">
              <span className="settings-label"><LockIcon size={16} /> Vault</span>
              <Button variant="secondary" size="xs" onClick={handleLockVault}>Lock Now</Button>
              <Button variant="secondary" size="xs" onClick={handleClearClipboard}>Clear Clipboard</Button>
            </div>
          </div>
        </section>

        {/* ─── STUN Servers ─── */}
        <section className="settings-section">
          <h2 className="settings-section__title">STUN Servers</h2>
          <div className="settings-card">
            <div className="stun-server-list">
              {(stunConfig?.servers || []).map((srv, i) => {
                // Find health info from diagnostics
                const diagServer = networkDiagnostics?.stun_servers?.find((d: any) => srv.includes(d.server) || d.server.includes(srv));
                const isHealthy = diagServer?.reachable;
                return (
                <div key={i} className="stun-server-item">
                  <div className="stun-health-item">
                    <span className={`stun-badge stun-badge--${isHealthy === true ? 'ok' : isHealthy === false ? 'fail' : 'unknown'}`}>
                      {isHealthy === true ? "OK" : isHealthy === false ? "FAIL" : "?"}
                    </span>
                    <span className="stun-health-item__server">{srv}</span>
                    {diagServer?.rtt_ms && <span className="stun-health-item__rtt">{diagServer.rtt_ms}ms</span>}
                  </div>
                  <button className="btn btn--icon btn--icon-sm" onClick={() => handleRemoveStunServer(i)} aria-label="Remove STUN server"><CloseIcon size={14} /></button>
                </div>
                );
              })}
            </div>
            <div className="settings-row">
              <Input placeholder="host:port" value={stunServerInput} onChange={e => setStunServerInput(e.target.value)} compact mono clearable onClear={() => setStunServerInput("")} />
              <Button size="xs" onClick={handleAddStunServer} disabled={!stunServerInput.trim()}>Add</Button>
              <Button variant="secondary" size="xs" onClick={handleResetStunDefaults} aria-label="Reset STUN servers to defaults">Reset</Button>
            </div>
          </div>
        </section>

        {/* ─── Theme ─── */}
        <section className="settings-section">
          <h2 className="settings-section__title">Theme</h2>
          <div className="settings-card">
            <div className="settings-row">
              <span className="settings-label">Appearance</span>
              <div className="theme-selector">
                <button className={`btn btn--icon btn--icon-sm ${theme === 'light' ? 'btn--icon-copied' : ''}`} onClick={() => setTheme('light')} aria-label="Light theme" title="Light">
                  <SunIcon size={18} />
                </button>
                <button className={`btn btn--icon btn--icon-sm ${theme === 'dark' ? 'btn--icon-copied' : ''}`} onClick={() => setTheme('dark')} aria-label="Dark theme" title="Dark">
                  <MoonIcon size={18} />
                </button>
                <button className={`btn btn--icon btn--icon-sm ${theme === 'system' ? 'btn--icon-copied' : ''}`} onClick={() => setTheme('system')} aria-label="System theme" title="System">
                  <MonitorIcon size={18} />
                </button>
              </div>
              <span className="settings-hint">Current: {resolvedTheme}</span>
            </div>
            <div className="settings-divider" />
            <div className="settings-row">
              <span className="settings-label">Accent Color</span>
              <input
                type="color"
                value={accentColor}
                onChange={(e) => setAccentColor(e.target.value)}
                className="color-picker"
                aria-label="Accent color"
              />
              <span className="settings-mono settings-mono--sm">{accentColor}</span>
              <Button size="xs" variant="secondary" onClick={() => setAccentColor("#6366f1")} aria-label="Reset accent color">Reset</Button>
            </div>
          </div>
        </section>

        {/* ─── About ─── */}
        <section className="settings-section">
          <h2 className="settings-section__title">About</h2>
          <div className="settings-card">
            <div className="settings-row">
              <span className="settings-label">Version</span>
              <span>{appVersion || "unknown"}</span>
            </div>
            <div className="settings-row">
              <span className="settings-label">Crypto</span>
              <span className="text-muted text-sm">Ed25519 · X25519 · XChaCha20-Poly1305 · X3DH · Double Ratchet</span>
            </div>
          </div>
        </section>
      </div>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
      </div>
    </div>
  );
}
