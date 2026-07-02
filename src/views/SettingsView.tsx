import { useState } from "react";
import { Button, Input, Badge, ToastContainer } from "../components/ui";
import { ArrowLeftIcon, GearIcon, CopyIcon, CheckIcon, CloseIcon, WifiIcon, GlobeIcon, LockIcon, EyeOffIcon, MonitorIcon, SunIcon, MoonIcon } from "../components/ui/Icons";
import { useApp } from "../context/AppContext";
import { useSettings } from "../context/SettingsContext";
import { useTheme } from "../context/ThemeContext";

export default function SettingsView() {
  const { identity, toasts, addToast, removeToast, setView } = useApp();
  const { theme, setTheme, resolvedTheme } = useTheme();
  const {
    networkSettings, publicIp, stunLoading, networkDiagnostics,
    stunConfig, stunServerInput, privateMode, connectivityResult,
    handleStunDiscover, handleAddStunServer,
    handleRemoveStunServer, handleResetStunDefaults, handlePrivateModeToggle,
    handleConnectivityCheck, handleTorToggle, setStunServerInput,
    discoveryConfig, discoveredPeers,
    handleLanToggle, handleDhtToggle, handleRefreshDiscovery,
    securityConfig,
    handleScreenCaptureToggle, handleClipboardClearSecsChange,
    handleIdleLockSecsChange, handleLockVault, handleClearClipboard,
    scheduleClipboardClear,
  } = useSettings();
  const [fpCopied, setFpCopied] = useState(false);
  const [ipCopied, setIpCopied] = useState(false);
  const [torEnabled, setTorEnabled] = useState(networkSettings?.tor_enabled ?? false);

  const onBackToHub = () => setView("hub");

  return (
    <div className="app-shell">
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

            <div className="settings-row">
              <span className="settings-label">Clipboard Auto-Clear</span>
              <div className="select-wrap" style={{ width: 'auto' }}>
                <select className="select--compact"
                  value={securityConfig?.clipboard_clear_secs ?? 0}
                  onChange={e => handleClipboardClearSecsChange(parseInt(e.target.value, 10))}
                  aria-label="Clipboard auto-clear timeout"
                >
                  <option value={0}>Off</option>
                  <option value={5}>5 seconds</option>
                  <option value={10}>10 seconds</option>
                  <option value={30}>30 seconds</option>
                  <option value={60}>1 minute</option>
                </select>
              </div>
              <span className="settings-hint">Auto-clear clipboard after copying sensitive data</span>
            </div>

            <div className="settings-row">
              <span className="settings-label">Idle Vault Lock</span>
              <div className="select-wrap" style={{ width: 'auto' }}>
                <select className="select--compact"
                  value={securityConfig?.idle_lock_secs ?? 0}
                  onChange={e => handleIdleLockSecsChange(parseInt(e.target.value, 10))}
                  aria-label="Idle vault lock timeout"
                >
                  <option value={0}>Off</option>
                  <option value={60}>1 minute</option>
                  <option value={300}>5 minutes</option>
                  <option value={600}>10 minutes</option>
                  <option value={1800}>30 minutes</option>
                </select>
              </div>
              <span className="settings-hint">Auto-lock vault after inactivity</span>
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
            {(stunConfig?.servers || []).map((srv, i) => (
              <div className="settings-row" key={i}>
                <span className="settings-mono">{srv}</span>
                <button className="btn btn--icon btn--icon-sm" onClick={() => handleRemoveStunServer(i)} aria-label="Remove STUN server"><CloseIcon size={14} /></button>
              </div>
            ))}
            <div className="settings-row">
              <Input placeholder="host:port" value={stunServerInput} onChange={e => setStunServerInput(e.target.value)} compact mono clearable onClear={() => setStunServerInput("")} />
              <Button size="xs" onClick={handleAddStunServer} disabled={!stunServerInput.trim()}>Add</Button>
              <Button variant="secondary" size="xs" onClick={handleResetStunDefaults}>Reset</Button>
            </div>
          </div>
        </section>

        {/* ─── About ─── */}
        <section className="settings-section">
          <h2 className="settings-section__title">About</h2>
          <div className="settings-card">
            <div className="settings-row">
              <span className="settings-label">Version</span>
              <span>2.5.x</span>
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
  );
}
