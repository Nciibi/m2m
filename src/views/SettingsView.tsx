import { useState } from "react";
import { ToastContainer } from "../components/ui";
import { useApp } from "../context/AppContext";
import { useSettings } from "../context/SettingsContext";
import { useTheme } from "../context/ThemeContext";

export default function SettingsView() {
  const { identity, toasts, removeToast, setView } = useApp();
  const { theme, setTheme, accentColor, setAccentColor } = useTheme();
  const {
    networkSettings, publicIp, stunLoading,
    privateMode,
    handleStunDiscover,
    handlePrivateModeToggle,
    handleTorToggle,
    discoveryConfig,
    handleLanToggle, handleDhtToggle,
    securityConfig,
    handleScreenCaptureToggle, handleClipboardClearSecsChange,
    handleIdleLockSecsChange, handleLockVault, handleClearClipboard,
  } = useSettings();
  
  const [fpCopied, setFpCopied] = useState(false);
  const [torEnabled, setTorEnabled] = useState(networkSettings?.tor_enabled ?? false);

  return (
    <div className="flex flex-col h-screen overflow-hidden w-full relative z-10 text-on-surface bg-transparent font-body-base">
      <style>{`
        .settings-glass-card {
            position: relative;
            background: var(--color-bg-surface);
            backdrop-filter: var(--glass-blur-xl) var(--glass-saturate);
            border: 1px solid var(--color-border-default);
            border-radius: 24px;
            box-shadow: var(--shadow-app-shell);
            transition: transform 0.4s cubic-bezier(0.34, 1.56, 0.64, 1), box-shadow 0.4s ease;
            overflow: hidden;
        }
        .settings-glass-card::before {
            content: "";
            position: absolute;
            top: 0; left: 0; right: 0; bottom: 0;
            border-radius: inherit;
            padding: 1px;
            background-image: radial-gradient(
                400px circle at var(--cursor-x) var(--cursor-y),
                var(--color-border-active),
                transparent 40%
            );
            background-attachment: fixed;
            -webkit-mask: linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
            -webkit-mask-composite: xor;
            mask-composite: exclude;
            pointer-events: none;
            opacity: 0;
            transition: opacity 0.5s ease;
        }
        .settings-glass-card:hover::before {
            opacity: 1;
        }
        .settings-glass-card:hover {
            transform: translateY(-4px) scale(1.01);
            box-shadow: var(--shadow-card-hover);
        }
        .mac-toggle {
            position: relative;
            width: 36px;
            height: 20px;
            background: var(--color-bg-input);
            border-radius: 999px;
            transition: background 0.2s ease;
        }
        .mac-toggle::after {
            content: '';
            position: absolute;
            top: 2px;
            left: 2px;
            width: 16px;
            height: 16px;
            background: white;
            border-radius: 50%;
            transition: transform 0.2s ease;
        }
        input:checked + .mac-toggle {
            background: var(--color-accent);
        }
        input:checked + .mac-toggle::after {
            transform: translateX(16px);
        }
        .segmented-btn {
            background: var(--color-bg-input);
            border: 1px solid var(--color-border-default);
        }
        .active-segment {
            background: var(--color-primary-glow);
            border: 1px solid var(--color-border-active);
            color: var(--color-text-accent);
        }
        .no-scrollbar::-webkit-scrollbar {
            display: none;
        }
        .no-scrollbar {
            -ms-overflow-style: none;
            scrollbar-width: none;
        }
      `}</style>

      {/* TopAppBar */}
      <header className="fixed top-0 w-full z-50 backdrop-blur-3xl border-b border-border-subtle bg-surface/60 shadow-sm">
        <div className="flex justify-between items-center px-lg py-md max-w-container-max mx-auto h-16">
          <div className="flex items-center gap-md">
            <span className="material-symbols-outlined text-primary text-2xl">settings</span>
            <h1 className="font-headline-2xl text-headline-2xl font-bold text-on-surface tracking-tight">Settings</h1>
          </div>
          <button onClick={() => setView("hub")} className="active:scale-95 duration-200 p-2 hover:bg-input-bg rounded-full transition-colors">
            <span className="material-symbols-outlined text-on-surface-variant">close</span>
          </button>
        </div>
      </header>

      {/* Scrollable Main Canvas */}
      <main className="flex-1 overflow-y-auto no-scrollbar pt-20 pb-xl px-gutter">
        <div className="max-w-[1000px] mx-auto flex flex-col gap-lg">
          
          {/* 1. IDENTITY */}
          <section className="settings-glass-card p-lg animate-in fade-in slide-in-from-bottom-2 duration-300">
            <div className="flex items-center gap-sm mb-md text-primary">
              <span className="material-symbols-outlined text-md">fingerprint</span>
              <h2 className="font-body-lg text-body-lg font-bold uppercase tracking-wider">Identity</h2>
            </div>
            <div className="space-y-md">
              <div className="flex flex-col gap-xs">
                <label className="font-label-sm text-text-muted">Node Fingerprint</label>
                <div className="flex items-center justify-between bg-input-bg p-md rounded-lg border border-border-subtle group hover:border-primary/50 transition-colors">
                  <code className="font-mono-code text-mono-code text-primary break-all">{identity?.fingerprint || "—"}</code>
                  <button onClick={() => {
                      if (identity?.fingerprint) {
                        navigator.clipboard.writeText(identity.fingerprint);
                        setFpCopied(true);
                        setTimeout(() => setFpCopied(false), 2000);
                      }
                    }} className="text-on-surface-variant hover:text-primary transition-colors shrink-0">
                    <span className="material-symbols-outlined text-lg">{fpCopied ? "check" : "content_copy"}</span>
                  </button>
                </div>
              </div>
              <div className="flex flex-col gap-xs">
                <label className="font-label-sm text-text-muted">Public Key</label>
                <div className="flex items-center justify-between bg-input-bg p-md rounded-lg border border-border-subtle">
                  <code className="font-mono-code text-mono-code truncate mr-4">{identity?.public_key_hex || "—"}</code>
                  <button className="text-on-surface-variant hover:text-primary transition-colors">
                    <span className="material-symbols-outlined text-lg">content_copy</span>
                  </button>
                </div>
              </div>
            </div>
          </section>

          {/* 2. THEME */}
          <section className="settings-glass-card p-lg animate-in fade-in slide-in-from-bottom-4 duration-500">
            <div className="flex items-center gap-sm mb-md text-primary">
              <span className="material-symbols-outlined text-md">palette</span>
              <h2 className="font-body-lg text-body-lg font-bold uppercase tracking-wider">Appearance</h2>
            </div>
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-md">
              <div>
                <p className="font-body-md text-on-surface mb-1">Interface Theme</p>
                <p className="font-label-sm text-text-muted">Current: {theme}</p>
              </div>
              <div className="flex bg-surface-container rounded-xl p-1 border border-border-subtle">
                <button onClick={() => setTheme('light')} className={`${theme === 'light' ? 'active-segment' : 'segmented-btn'} p-2 rounded-lg flex items-center justify-center transition-all w-12`}><span className="material-symbols-outlined">light_mode</span></button>
                <button onClick={() => setTheme('dark')} className={`${theme === 'dark' ? 'active-segment' : 'segmented-btn'} p-2 rounded-lg flex items-center justify-center transition-all w-12`}><span className="material-symbols-outlined">dark_mode</span></button>
                <button onClick={() => setTheme('system')} className={`${theme === 'system' ? 'active-segment' : 'segmented-btn'} p-2 rounded-lg flex items-center justify-center transition-all w-12`}><span className="material-symbols-outlined">desktop_windows</span></button>
              </div>
            </div>
            <div className="h-[1px] bg-border-subtle my-lg"></div>
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-md">
                <p className="font-body-md text-on-surface">Accent Color</p>
                <div className="flex items-center gap-2">
                   <div className="w-6 h-6 rounded-full ring-2 ring-white/20" style={{ background: accentColor }}></div>
                   <input type="color" value={accentColor} onChange={(e) => setAccentColor(e.target.value)} className="w-6 h-6 opacity-0 absolute cursor-pointer" />
                </div>
              </div>
              <button onClick={() => setAccentColor('#6366f1')} className="font-label-sm text-text-muted hover:text-primary transition-colors underline decoration-dotted">Reset</button>
            </div>
          </section>

          {/* 3. NETWORK */}
          <section className="settings-glass-card p-lg animate-in fade-in slide-in-from-bottom-6 duration-700">
            <div className="flex items-center gap-sm mb-md text-primary">
              <span className="material-symbols-outlined text-md">lan</span>
              <h2 className="font-body-lg text-body-lg font-bold uppercase tracking-wider">Network</h2>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-lg mb-lg">
              <div className="space-y-md">
                <div className="flex justify-between items-center text-sm">
                  <span className="text-text-muted">Public IP</span>
                  <span className="font-mono-code text-on-surface flex items-center gap-2">{publicIp || "Unknown"} <span className="material-symbols-outlined text-xs cursor-pointer hover:text-primary">content_copy</span></span>
                </div>
                <div className="flex justify-between items-center text-sm">
                  <span className="text-text-muted">NAT Type</span>
                  <span className="font-body-md text-on-surface">RestrictedCone</span>
                </div>
                <div className="flex justify-between items-center text-sm">
                  <span className="text-text-muted">STUN Status</span>
                  <span className="font-body-md text-tertiary">3/4 reachable</span>
                </div>
              </div>
              <div className="space-y-md">
                <div className="flex items-center justify-between">
                  <span className="font-body-md">Private Mode</span>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input checked={privateMode} onChange={handlePrivateModeToggle} className="sr-only" type="checkbox"/>
                    <div className="mac-toggle"></div>
                  </label>
                </div>
                <div className="flex items-center justify-between">
                  <span className="font-body-md">Tor Routing</span>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input checked={torEnabled} onChange={async () => { await handleTorToggle(); setTorEnabled(!torEnabled); }} className="sr-only" type="checkbox"/>
                    <div className="mac-toggle"></div>
                  </label>
                </div>
              </div>
            </div>
            <div className="flex gap-md">
              <button className="flex-1 py-2 px-md border border-border-subtle rounded-lg font-label-sm text-on-surface-variant hover:bg-input-bg transition-all">Test Tor</button>
              <button onClick={handleStunDiscover} className="flex-1 py-2 px-md border border-border-subtle rounded-lg font-label-sm text-on-surface-variant hover:bg-input-bg transition-all">{stunLoading ? "Checking..." : "Check Connectivity"}</button>
            </div>
          </section>

          {/* 4. DISCOVERY */}
          <section className="settings-glass-card p-lg">
            <div className="flex items-center gap-sm mb-md text-primary">
              <span className="material-symbols-outlined text-md">radar</span>
              <h2 className="font-body-lg text-body-lg font-bold uppercase tracking-wider">Discovery</h2>
            </div>
            <div className="space-y-md mb-md">
              <div className="flex items-center justify-between">
                <div>
                  <p className="font-body-md">LAN Discovery</p>
                  <p className="text-xs text-text-muted">Search local network for peers</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input checked={discoveryConfig?.lan_enabled ?? false} onChange={handleLanToggle} className="sr-only" type="checkbox"/>
                  <div className="mac-toggle"></div>
                </label>
              </div>
              <div className="flex items-center justify-between">
                <div>
                  <p className="font-body-md">DHT Discovery</p>
                  <p className="text-xs text-text-muted">Distributed Hash Table global search</p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input checked={discoveryConfig?.dht_enabled ?? false} onChange={handleDhtToggle} className="sr-only" type="checkbox"/>
                  <div className="mac-toggle"></div>
                </label>
              </div>
            </div>
            <div className="flex items-start gap-sm bg-warning/5 border border-warning/20 p-md rounded-lg mt-md">
              <span className="material-symbols-outlined text-warning text-md">report_problem</span>
              <p className="text-xs text-warning/90 leading-relaxed">Enabling discovery may reveal your node presence to other entities in the same network segments.</p>
            </div>
          </section>

          {/* 5. SECURITY */}
          <section className="settings-glass-card p-lg">
            <div className="flex items-center gap-sm mb-md text-primary">
              <span className="material-symbols-outlined text-md">security</span>
              <h2 className="font-body-lg text-body-lg font-bold uppercase tracking-wider">Security</h2>
            </div>
            <div className="space-y-lg">
              <div className="flex items-center justify-between">
                <span className="font-body-md">Screen Capture Protection</span>
                <label className="relative inline-flex items-center cursor-pointer">
                  <input checked={securityConfig?.screen_capture_protection ?? false} onChange={handleScreenCaptureToggle} className="sr-only" type="checkbox"/>
                  <div className="mac-toggle"></div>
                </label>
              </div>
              <div className="grid grid-cols-2 gap-md">
                <div className="flex flex-col gap-xs">
                  <label className="font-label-sm text-text-muted">Clipboard Auto-Clear</label>
                  <select value={securityConfig?.clipboard_clear_secs ?? 0} onChange={e => handleClipboardClearSecsChange(parseInt(e.target.value, 10))} className="bg-surface-variant border border-border-subtle rounded-lg text-sm text-on-surface focus:ring-primary focus:border-primary px-3 py-2">
                    <option value={0}>Off</option>
                    <option value={5}>5 seconds</option>
                    <option value={10}>10 seconds</option>
                    <option value={30}>30 seconds</option>
                    <option value={60}>1 minute</option>
                  </select>
                </div>
                <div className="flex flex-col gap-xs">
                  <label className="font-label-sm text-text-muted">Idle Vault Lock</label>
                  <select value={securityConfig?.idle_lock_secs ?? 0} onChange={e => handleIdleLockSecsChange(parseInt(e.target.value, 10))} className="bg-surface-variant border border-border-subtle rounded-lg text-sm text-on-surface focus:ring-primary focus:border-primary px-3 py-2">
                    <option value={0}>Off</option>
                    <option value={60}>1 minute</option>
                    <option value={600}>10 minutes</option>
                    <option value={1800}>30 minutes</option>
                    <option value={3600}>1 hour</option>
                  </select>
                </div>
              </div>
              <div className="h-[1px] bg-border-subtle"></div>
              <div className="flex gap-md">
                <button onClick={handleLockVault} className="flex-1 py-2 px-md border border-danger/30 text-danger rounded-lg font-label-sm hover:bg-danger/10 transition-all flex items-center justify-center gap-2">
                  <span className="material-symbols-outlined text-sm">lock</span> Lock Now
                </button>
                <button onClick={handleClearClipboard} className="flex-1 py-2 px-md border border-border-subtle text-on-surface-variant rounded-lg font-label-sm hover:bg-input-bg transition-all flex items-center justify-center gap-2">
                  <span className="material-symbols-outlined text-sm">delete</span> Clear Clipboard
                </button>
              </div>
            </div>
          </section>

          {/* 6. ABOUT */}
          <section className="settings-glass-card p-lg mb-xl">
            <div className="flex items-center gap-sm mb-md text-primary">
              <span className="material-symbols-outlined text-md">info</span>
              <h2 className="font-body-lg text-body-lg font-bold uppercase tracking-wider">About</h2>
            </div>
            <div className="flex justify-between items-end">
              <div>
                <p className="font-headline-2xl font-bold tracking-tight text-on-surface">M2M Messenger</p>
                <p className="font-label-sm text-text-muted mb-md">Version 2.5.x Stable Build</p>
                <div className="flex gap-2">
                  <span className="px-2 py-1 bg-secondary-container/30 border border-secondary-container text-[10px] rounded font-mono-label text-secondary uppercase">Ed25519</span>
                  <span className="px-2 py-1 bg-secondary-container/30 border border-secondary-container text-[10px] rounded font-mono-label text-secondary uppercase">X25519</span>
                  <span className="px-2 py-1 bg-secondary-container/30 border border-secondary-container text-[10px] rounded font-mono-label text-secondary uppercase">XChaCha20</span>
                </div>
              </div>
              <div className="w-16 h-16 opacity-20 hover:opacity-100 transition-opacity duration-700">
                <div className="w-full h-full bg-gradient-to-br from-primary to-tertiary rounded-xl rotate-12"></div>
              </div>
            </div>
          </section>

        </div>
      </main>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
