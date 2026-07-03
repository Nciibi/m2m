import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ToastContainer } from "../components/ui";
import { ArrowLeftIcon, GearIcon, CopyIcon, CheckIcon, CloseIcon, WifiIcon, GlobeIcon, LockIcon, EyeOffIcon, MonitorIcon, SunIcon, MoonIcon, RefreshIcon } from "../components/ui/Icons";
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
    handleScreenCaptureToggle, handleClipboardClearSecsChange,
    handleIdleLockSecsChange, handleLockVault, handleClearClipboard,
    scheduleClipboardClear,
  } = useSettings();
  
  const [fpCopied, setFpCopied] = useState(false);
  const [ipCopied, setIpCopied] = useState(false);
  const [torEnabled, setTorEnabled] = useState(networkSettings?.tor_enabled ?? false);
  const [activeSection, setActiveSection] = useState("identity");

  const onBackToHub = () => setView("hub");

  // Scroll spy effect could go here, for simplicity we just map clicks to sections
  const scrollToSection = (id: string) => {
    setActiveSection(id);
    document.getElementById(`section-${id}`)?.scrollIntoView({ behavior: 'smooth' });
  };

  const navItems = [
    { id: 'identity', icon: 'fingerprint', label: 'Identity' },
    { id: 'security', icon: 'security', label: 'Security' },
    { id: 'network', icon: 'router', label: 'Network & Discovery' },
    { id: 'appearance', icon: 'palette', label: 'Appearance' },
    { id: 'about', icon: 'info', label: 'About M2M' },
  ];

  return (
    <div style={{ display: 'flex', width: '100%', minHeight: '100vh', alignItems: 'center', justifyContent: 'center', background: 'var(--color-bg-dark)', overflow: 'hidden' }}>
      
      <main className="app-shell" style={{ maxWidth: '1000px', flexDirection: 'column' }}>
        
        {/* Settings Header */}
        <header style={{ height: '64px', padding: '0 24px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid rgba(255, 255, 255, 0.08)', flexShrink: 0, background: 'rgba(255, 255, 255, 0.02)' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '16px' }}>
            <div style={{ width: '32px', height: '32px', borderRadius: '8px', background: 'rgba(255, 255, 255, 0.05)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
              <span className="material-symbols-outlined" style={{ fontSize: '18px', color: 'var(--color-text-primary)' }}>settings</span>
            </div>
            <h1 style={{ fontSize: '18px', fontWeight: 600, color: 'var(--color-text-primary)', margin: 0 }}>Settings</h1>
          </div>
          <button onClick={onBackToHub} style={{ background: 'var(--color-primary)', color: 'var(--color-bg-dark)', border: 'none', padding: '8px 16px', borderRadius: '8px', fontWeight: 600, fontSize: '14px', cursor: 'pointer', transition: 'all 0.2s' }}>
            Done
          </button>
        </header>

        {/* Two-Column Content */}
        <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
          
          {/* Local Sidebar */}
          <nav style={{ width: '240px', borderRight: '1px solid rgba(255, 255, 255, 0.08)', display: 'flex', flexDirection: 'column', gap: '4px', padding: '24px 12px', flexShrink: 0, overflowY: 'auto' }}>
            {navItems.map(item => (
              <button 
                key={item.id}
                onClick={() => scrollToSection(item.id)}
                style={{ 
                  display: 'flex', alignItems: 'center', gap: '12px', padding: '10px 12px', borderRadius: '8px', 
                  background: activeSection === item.id ? 'rgba(99, 102, 241, 0.1)' : 'transparent',
                  color: activeSection === item.id ? 'var(--color-primary)' : 'var(--color-text-secondary)',
                  border: 'none', cursor: 'pointer', textAlign: 'left', fontWeight: activeSection === item.id ? 600 : 500,
                  transition: 'all 0.2s'
                }}
              >
                <span className="material-symbols-outlined" style={{ fontSize: '20px' }}>{item.icon}</span>
                {item.label}
              </button>
            ))}
          </nav>

          {/* Scrolling Content Area */}
          <div style={{ flex: 1, padding: '32px', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '40px' }} onScroll={(e) => {
            // Simple scroll spy logic can be implemented here if needed.
          }}>
            
            {/* Identity Section */}
            <section id="section-identity" style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div>
                <h2 style={{ fontSize: '20px', fontWeight: 600, color: 'var(--color-text-primary)', margin: '0 0 4px 0' }}>Identity</h2>
                <p style={{ margin: 0, fontSize: '14px', color: 'var(--color-text-muted)' }}>Your cryptographic identity and fingerprint.</p>
              </div>
              <div style={{ background: 'rgba(255, 255, 255, 0.02)', borderRadius: '16px', border: '1px solid rgba(255, 255, 255, 0.05)', overflow: 'hidden' }}>
                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid rgba(255, 255, 255, 0.05)' }}>
                  <div>
                    <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>Fingerprint</div>
                    <div style={{ fontFamily: 'var(--font-mono)', fontSize: '13px', color: 'var(--color-primary)', marginTop: '4px' }}>{identity?.fingerprint || "—"}</div>
                  </div>
                  <button className="icon-btn" onClick={() => {
                    if (identity?.fingerprint) {
                      navigator.clipboard.writeText(identity.fingerprint);
                      setFpCopied(true);
                      setTimeout(() => setFpCopied(false), 2000);
                    }
                  }}>
                    {fpCopied ? <CheckIcon size={20} color="var(--color-success)" /> : <CopyIcon size={20} />}
                  </button>
                </div>
                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <div style={{ minWidth: 0 }}>
                    <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>Public Key</div>
                    <div style={{ fontFamily: 'var(--font-mono)', fontSize: '13px', color: 'var(--color-text-secondary)', marginTop: '4px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{identity?.public_key_hex || "—"}</div>
                  </div>
                </div>
              </div>
            </section>

            {/* Security Section */}
            <section id="section-security" style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div>
                <h2 style={{ fontSize: '20px', fontWeight: 600, color: 'var(--color-text-primary)', margin: '0 0 4px 0' }}>Security</h2>
                <p style={{ margin: 0, fontSize: '14px', color: 'var(--color-text-muted)' }}>Protect your local vault and application data.</p>
              </div>
              <div style={{ background: 'rgba(255, 255, 255, 0.02)', borderRadius: '16px', border: '1px solid rgba(255, 255, 255, 0.05)', overflow: 'hidden' }}>
                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid rgba(255, 255, 255, 0.05)' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                    <div style={{ width: '32px', height: '32px', borderRadius: '8px', background: 'rgba(239, 68, 68, 0.1)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                      <span className="material-symbols-outlined" style={{ fontSize: '16px', color: 'var(--color-danger)' }}>visibility_off</span>
                    </div>
                    <div>
                      <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>Screen Capture Protection</div>
                      <div style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>Prevent window from appearing in screenshots</div>
                    </div>
                  </div>
                  <label className="toggle">
                    <input type="checkbox" checked={securityConfig?.screen_capture_protection ?? false} onChange={handleScreenCaptureToggle} />
                    <span className="toggle-slider" />
                  </label>
                </div>

                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid rgba(255, 255, 255, 0.05)' }}>
                  <div>
                    <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>Clipboard Auto-Clear</div>
                    <div style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>Clear sensitive data from clipboard after delay</div>
                  </div>
                  <select className="select--compact" value={securityConfig?.clipboard_clear_secs ?? 0} onChange={e => handleClipboardClearSecsChange(parseInt(e.target.value, 10))} style={{ background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.1)', color: 'white', padding: '6px 12px', borderRadius: '8px', outline: 'none' }}>
                    <option value={0}>Off</option>
                    <option value={5}>5s</option>
                    <option value={10}>10s</option>
                    <option value={30}>30s</option>
                    <option value={60}>1m</option>
                  </select>
                </div>

                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid rgba(255, 255, 255, 0.05)' }}>
                  <div>
                    <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>Idle Vault Lock</div>
                    <div style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>Auto-lock after inactivity</div>
                  </div>
                  <select className="select--compact" value={securityConfig?.idle_lock_secs ?? 0} onChange={e => handleIdleLockSecsChange(parseInt(e.target.value, 10))} style={{ background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.1)', color: 'white', padding: '6px 12px', borderRadius: '8px', outline: 'none' }}>
                    <option value={0}>Off</option>
                    <option value={60}>1m</option>
                    <option value={300}>5m</option>
                    <option value={600}>10m</option>
                    <option value={1800}>30m</option>
                  </select>
                </div>

                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <div style={{ display: 'flex', gap: '12px' }}>
                    <button onClick={handleLockVault} style={{ background: 'rgba(255,255,255,0.05)', color: 'white', border: '1px solid rgba(255,255,255,0.1)', padding: '6px 16px', borderRadius: '8px', cursor: 'pointer', fontSize: '13px' }}>Lock Vault Now</button>
                    <button onClick={handleClearClipboard} style={{ background: 'rgba(255,255,255,0.05)', color: 'white', border: '1px solid rgba(255,255,255,0.1)', padding: '6px 16px', borderRadius: '8px', cursor: 'pointer', fontSize: '13px' }}>Clear Clipboard</button>
                  </div>
                </div>
              </div>
            </section>

            {/* Network Section */}
            <section id="section-network" style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div>
                <h2 style={{ fontSize: '20px', fontWeight: 600, color: 'var(--color-text-primary)', margin: '0 0 4px 0' }}>Network & Discovery</h2>
                <p style={{ margin: 0, fontSize: '14px', color: 'var(--color-text-muted)' }}>Configure how you connect to peers.</p>
              </div>
              <div style={{ background: 'rgba(255, 255, 255, 0.02)', borderRadius: '16px', border: '1px solid rgba(255, 255, 255, 0.05)', overflow: 'hidden' }}>
                
                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid rgba(255, 255, 255, 0.05)' }}>
                  <div>
                    <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>Private Mode</div>
                    <div style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>Hide IP from generated invites</div>
                  </div>
                  <label className="toggle">
                    <input type="checkbox" checked={privateMode} onChange={handlePrivateModeToggle} />
                    <span className="toggle-slider" />
                  </label>
                </div>

                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid rgba(255, 255, 255, 0.05)' }}>
                  <div>
                    <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>Tor Routing</div>
                    <div style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>Route outbound connections via Tor (requires restart)</div>
                  </div>
                  <label className="toggle">
                    <input type="checkbox" checked={torEnabled} onChange={async () => { await handleTorToggle(); setTorEnabled(!torEnabled); }} />
                    <span className="toggle-slider" />
                  </label>
                </div>

                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid rgba(255, 255, 255, 0.05)' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                    <div style={{ width: '32px', height: '32px', borderRadius: '8px', background: 'rgba(16, 185, 129, 0.1)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                      <span className="material-symbols-outlined" style={{ fontSize: '16px', color: 'var(--color-success)' }}>wifi</span>
                    </div>
                    <div>
                      <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>LAN Discovery</div>
                      <div style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>Broadcast presence on local network</div>
                    </div>
                  </div>
                  <label className="toggle">
                    <input type="checkbox" checked={discoveryConfig?.lan_enabled ?? false} onChange={handleLanToggle} />
                    <span className="toggle-slider" />
                  </label>
                </div>

                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid rgba(255, 255, 255, 0.05)' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                    <div style={{ width: '32px', height: '32px', borderRadius: '8px', background: 'rgba(245, 158, 11, 0.1)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                      <span className="material-symbols-outlined" style={{ fontSize: '16px', color: 'var(--color-warning)' }}>public</span>
                    </div>
                    <div>
                      <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>DHT Discovery</div>
                      <div style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>Discover peers globally via DHT</div>
                    </div>
                  </div>
                  <label className="toggle">
                    <input type="checkbox" checked={discoveryConfig?.dht_enabled ?? false} onChange={handleDhtToggle} />
                    <span className="toggle-slider" />
                  </label>
                </div>

                <div style={{ padding: '16px', display: 'flex', flexDirection: 'column', gap: '12px', borderBottom: '1px solid rgba(255, 255, 255, 0.05)' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>Public IP Check</div>
                    <button onClick={handleStunDiscover} disabled={stunLoading} style={{ background: 'rgba(255,255,255,0.05)', color: 'white', border: '1px solid rgba(255,255,255,0.1)', padding: '4px 12px', borderRadius: '8px', cursor: 'pointer', fontSize: '12px' }}>
                      {stunLoading ? 'Checking...' : 'Check Now'}
                    </button>
                  </div>
                  {publicIp && (
                    <div style={{ fontFamily: 'var(--font-mono)', fontSize: '13px', color: 'var(--color-success)', background: 'rgba(16, 185, 129, 0.05)', padding: '8px 12px', borderRadius: '8px', border: '1px solid rgba(16, 185, 129, 0.1)' }}>
                      Detected IP: {publicIp}
                    </div>
                  )}
                </div>

              </div>
            </section>

            {/* Appearance Section */}
            <section id="section-appearance" style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div>
                <h2 style={{ fontSize: '20px', fontWeight: 600, color: 'var(--color-text-primary)', margin: '0 0 4px 0' }}>Appearance</h2>
                <p style={{ margin: 0, fontSize: '14px', color: 'var(--color-text-muted)' }}>Customize the visual style of M2M.</p>
              </div>
              <div style={{ background: 'rgba(255, 255, 255, 0.02)', borderRadius: '16px', border: '1px solid rgba(255, 255, 255, 0.05)', overflow: 'hidden' }}>
                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid rgba(255, 255, 255, 0.05)' }}>
                  <div>
                    <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>Theme Preference</div>
                  </div>
                  <div style={{ display: 'flex', background: 'rgba(255,255,255,0.05)', borderRadius: '8px', padding: '4px', gap: '4px' }}>
                    <button onClick={() => setTheme('light')} style={{ background: theme === 'light' ? 'rgba(255,255,255,0.1)' : 'transparent', color: theme === 'light' ? 'white' : 'var(--color-text-muted)', border: 'none', padding: '6px 12px', borderRadius: '6px', display: 'flex', alignItems: 'center', gap: '6px', cursor: 'pointer' }}>
                      <SunIcon size={14} /> Light
                    </button>
                    <button onClick={() => setTheme('dark')} style={{ background: theme === 'dark' ? 'rgba(255,255,255,0.1)' : 'transparent', color: theme === 'dark' ? 'white' : 'var(--color-text-muted)', border: 'none', padding: '6px 12px', borderRadius: '6px', display: 'flex', alignItems: 'center', gap: '6px', cursor: 'pointer' }}>
                      <MoonIcon size={14} /> Dark
                    </button>
                    <button onClick={() => setTheme('system')} style={{ background: theme === 'system' ? 'rgba(255,255,255,0.1)' : 'transparent', color: theme === 'system' ? 'white' : 'var(--color-text-muted)', border: 'none', padding: '6px 12px', borderRadius: '6px', display: 'flex', alignItems: 'center', gap: '6px', cursor: 'pointer' }}>
                      <MonitorIcon size={14} /> Auto
                    </button>
                  </div>
                </div>
                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <div>
                    <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>Accent Color</div>
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                    <span style={{ fontFamily: 'var(--font-mono)', fontSize: '12px', color: 'var(--color-text-muted)' }}>{accentColor}</span>
                    <input type="color" value={accentColor} onChange={(e) => setAccentColor(e.target.value)} style={{ width: '32px', height: '32px', border: 'none', borderRadius: '8px', cursor: 'pointer', background: 'none' }} />
                  </div>
                </div>
              </div>
            </section>

            {/* About Section */}
            <section id="section-about" style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div>
                <h2 style={{ fontSize: '20px', fontWeight: 600, color: 'var(--color-text-primary)', margin: '0 0 4px 0' }}>About M2M</h2>
                <p style={{ margin: 0, fontSize: '14px', color: 'var(--color-text-muted)' }}>System information and version.</p>
              </div>
              <div style={{ background: 'rgba(255, 255, 255, 0.02)', borderRadius: '16px', border: '1px solid rgba(255, 255, 255, 0.05)', overflow: 'hidden' }}>
                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid rgba(255, 255, 255, 0.05)' }}>
                  <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>Version</div>
                  <div style={{ color: 'var(--color-text-secondary)', fontSize: '14px' }}>2.5.x (Obsidian Prism)</div>
                </div>
                <div style={{ padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <div style={{ fontWeight: 500, color: 'var(--color-text-primary)' }}>Cryptographic Stack</div>
                  <div style={{ color: 'var(--color-text-secondary)', fontSize: '13px', textAlign: 'right' }}>Ed25519 · X25519<br/>XChaCha20-Poly1305 · Double Ratchet</div>
                </div>
              </div>
            </section>

          </div>
        </div>
      </main>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
