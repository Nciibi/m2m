import { useState } from "react";
import { Button, Input, Badge, ToastContainer } from "../components/ui";
import { ArrowLeftIcon, GearIcon, CopyIcon, CheckIcon, CloseIcon, WifiIcon, GlobeIcon } from "../components/ui/Icons";
import { useApp } from "../context/AppContext";
import { useSettings } from "../context/SettingsContext";

export default function SettingsView() {
  const { identity, toasts, removeToast, setView } = useApp();
  const {
    networkSettings, publicIp, stunLoading, networkDiagnostics,
    stunConfig, stunServerInput, privateMode, connectivityResult,
    handleStunDiscover, handleAddStunServer,
    handleRemoveStunServer, handleResetStunDefaults, handlePrivateModeToggle,
    handleConnectivityCheck, handleTorToggle, setStunServerInput,
    discoveryConfig, discoveredPeers,
    handleLanToggle, handleDhtToggle, handleRefreshDiscovery,
  } = useSettings();
  const [fpCopied, setFpCopied] = useState(false);
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
                if (identity?.fingerprint) { navigator.clipboard.writeText(identity.fingerprint); setFpCopied(true); setTimeout(() => setFpCopied(false), 2000); }
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
