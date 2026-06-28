import { useState } from "react";
import { Button, Input, Badge, ToastContainer } from "../components/ui";
import { ArrowLeftIcon, GearIcon, CopyIcon, CheckIcon, CloseIcon, AlertTriangleIcon } from "../components/ui/Icons";
import type { Toast, IdentityInfo, NetworkSettings, StunConfig, NatTypeInfo } from "../types";

interface Props {
  identity: IdentityInfo | null;
  networkSettings: NetworkSettings | null;
  publicIp: string | null;
  stunLoading: boolean;
  networkDiagnostics: NatTypeInfo | null;
  stunConfig: StunConfig | null;
  stunServerInput: string;
  privateMode: boolean;
  connectivityResult: any;
  toasts: Toast[];
  removeToast: (id: string) => void;
  onBackToHub: () => void;
  onStunDiscover: () => Promise<void>;
  onAddStunServer: () => Promise<void>;
  onRemoveStunServer: (idx: number) => Promise<void>;
  onResetStunDefaults: () => Promise<void>;
  onPrivateModeToggle: () => Promise<void>;
  onConnectivityCheck: () => Promise<void>;
  onTorToggle: () => Promise<void>;
  setStunServerInput: (v: string) => void;
}

export default function SettingsView({
  identity, networkSettings, publicIp, stunLoading, networkDiagnostics,
  stunConfig, stunServerInput, privateMode, connectivityResult,
  toasts, removeToast, onBackToHub, onStunDiscover, onAddStunServer,
  onRemoveStunServer, onResetStunDefaults, onPrivateModeToggle,
  onConnectivityCheck, onTorToggle, setStunServerInput,
}: Props) {
  const [ipCopied, setIpCopied] = useState(false);
  const [fpCopied, setFpCopied] = useState(false);

  return (
    <div className="app-shell">
      <div className="app-header">
        <h1 className="app-header__title">
          <span className="app-header__icon-bg app-header__icon-bg--default">
            <GearIcon size={18} />
          </span>
          Settings
        </h1>
        <Button variant="secondary" size="sm" onClick={onBackToHub} id="back-to-hub-btn"><ArrowLeftIcon size={16} /> Back</Button>
      </div>

      <div className="app-content--scroll">
        {/* Public IP */}
        <div className="settings-section">
          <h3 className="section-header">Public IP & Connectivity</h3>
          <div className="settings-row">
            <div className="settings-label">
              <div className="settings-label__title">Public Address</div>
              <div className="settings-label__desc">Discovered via STUN — needed for invites across the internet.</div>
            </div>
            <div className="settings-value">
              {publicIp ? (
                <span className="mono-value mono-value--interactive" id="public-ip-display">
                  {publicIp}
                  <button onClick={() => { navigator.clipboard.writeText(publicIp); setIpCopied(true); setTimeout(() => setIpCopied(false), 2000); }}
                    aria-label="Copy IP" className="input__clear">
                    {ipCopied ? <span className="copied-pop"><CheckIcon size={14} /></span> : <CopyIcon size={14} />}
                  </button>
                </span>
              ) : <span className="text-muted">Not discovered</span>}
              <Button variant="secondary" size="sm" onClick={onStunDiscover} disabled={stunLoading} id="stun-discover-btn" loading={stunLoading}>STUN Discover</Button>
            </div>
          </div>
          <div className="settings-row">
            <div className="settings-label">
              <div className="settings-label__title">Connectivity Check</div>
              <div className="settings-label__desc">Verify your listening port is reachable.</div>
            </div>
            <div className="settings-value">
              <Button variant="secondary" size="sm" onClick={onConnectivityCheck} id="connectivity-check-btn">Check</Button>
            </div>
          </div>
          {connectivityResult && (
            <div className={`connectivity-result connectivity-result--${connectivityResult.reachable ? "success" : "warning"}`}>
              <strong>{connectivityResult.reachable ? "Reachable" : "Limited Reachability"}</strong>
              <div className="connectivity-details">
                <div>NAT Type: <code>{connectivityResult.nat_type}</code></div>
                {connectivityResult.behind_symmetric_nat && <div className="nat-warning">Symmetric NAT detected — inbound may fail without TURN relay.</div>}
              </div>
            </div>
          )}
        </div>

        {/* STUN Servers */}
        <div className="settings-section">
          <h3 className="section-header">STUN Servers</h3>
          <p className="settings-label__desc settings-label__desc--spaced">Configure STUN servers for IP discovery.</p>
          {stunConfig && (
            <>
              <div className="stun-server-list">
                {stunConfig.servers.map((s, i) => (
                  <div key={i} className="stun-server-item">
                    <span className="mono-value mono-value--plain">{s}</span>
                    <button className="btn btn--ghost btn--icon-sm"
                      onClick={() => onRemoveStunServer(i)} aria-label={`Remove ${s}`}><CloseIcon size={14} /></button>
                  </div>
                ))}
              </div>
              <div className="stun-server-add">
                <Input placeholder="host:port" value={stunServerInput} onChange={e => setStunServerInput(e.target.value)} onKeyDown={e => e.key === "Enter" && onAddStunServer()} id="stun-server-input" mono compact />
                <Button variant="secondary" size="sm" onClick={onAddStunServer} id="add-stun-server-btn">Add</Button>
                <Button variant="secondary" size="sm" onClick={onResetStunDefaults} id="reset-stun-btn">Reset Defaults</Button>
              </div>
            </>
          )}
        </div>

        {/* Diagnostics */}
        {networkDiagnostics && (
          <div className="settings-section">
            <h3 className="section-header">Network Diagnostics</h3>
            <div className="diag-grid">
              <div className="diag-item">
                <span className="diag-item__label">NAT Type</span>
                <Badge variant={networkDiagnostics.nat_type === "symmetric" ? "warning" : networkDiagnostics.nat_type === "blocked" ? "danger" : networkDiagnostics.nat_type === "unknown" ? "default" : "success"} compact>{networkDiagnostics.nat_type}</Badge>
              </div>
              <div className="diag-item">
                <span className="diag-item__label">Candidates</span>
                <span className="diag-item__value">{networkDiagnostics.candidates?.length || 0}</span>
              </div>
              <div className="diag-item diag-item--full">
                <span className="diag-item__label">STUN Servers</span>
                <div className="diag-item__stun">
                  {networkDiagnostics.stun_servers?.map((s: any, i: number) => (
                    <div key={i} className={`stun-health-item stun-health-item--${s.reachable ? "ok" : "fail"}`}>
                      <span>{s.reachable ? <CheckIcon size={14} color="var(--color-success)" /> : <CloseIcon size={14} color="var(--color-danger)" />}</span>
                      <code className="stun-health-item__server">{s.server}</code>
                      {s.rtt_ms && <span className="stun-health-item__rtt">{s.rtt_ms}ms</span>}
                    </div>
                  ))}
                </div>
              </div>
              {networkDiagnostics.candidates?.length > 0 && (
                <div className="diag-item diag-item--full">
                  <span className="diag-item__label">Candidates (by priority)</span>
                  <div className="candidate-list">
                    {networkDiagnostics.candidates.map((c: any, i: number) => (
                      <div key={i} className={`candidate-item candidate-item--${c.candidate_type === 0 ? "host" : c.candidate_type === 1 ? "srflx" : c.candidate_type === 2 ? "prflx" : "relay"}`}>
                        <span className="candidate-type">{c.candidate_type === 0 ? "Host" : c.candidate_type === 1 ? "SRFLX" : c.candidate_type === 2 ? "PRFLX" : "Relay"}</span>
                        <code className="candidate-item__address">{c.address}</code>
                        <span className="candidate-item__priority">prio: {c.priority}</span>
                        {i === 0 && <span className="candidate-item__active">Active</span>}
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </div>
        )}

        {/* Privacy */}
        <div className="settings-section">
          <h3 className="section-header">Privacy</h3>
          <div className="settings-row">
            <div className="settings-label">
              <div className="settings-label__title">Private Mode</div>
              <div className="settings-label__desc">Hide your public IP from invites. Only local addresses shared.</div>
            </div>
            <div className="settings-value">
              <Badge variant={privateMode ? "success" : "default"} compact dot>{privateMode ? "Enabled" : "Disabled"}</Badge>
              <Button variant={privateMode ? "danger" : "secondary"} size="sm" onClick={onPrivateModeToggle} id="private-mode-toggle">{privateMode ? "Disable" : "Enable"}</Button>
            </div>
          </div>
        </div>

        {/* Tor */}
        <div className="settings-section">
          <h3 className="section-header">Tor Routing</h3>
          <div className="settings-row">
            <div className="settings-label">
              <div className="settings-label__title">Tor Routing</div>
              <div className="settings-label__desc">Route outgoing connections through Tor SOCKS5.</div>
            </div>
            <div className="settings-value">
              <Badge variant={networkSettings?.tor_reachable ? "success" : "default"} compact dot={!!networkSettings?.tor_reachable}>
                {networkSettings?.tor_reachable ? "Proxy reachable" : "Proxy not found"}
              </Badge>
              <Button variant={networkSettings?.tor_enabled ? "danger" : "secondary"} size="sm" onClick={onTorToggle} id="tor-toggle-btn">
                {networkSettings?.tor_enabled ? "Disable Tor" : "Enable Tor"}
              </Button>
            </div>
          </div>
        </div>

        {/* Identity */}
        <div className="settings-section">
          <h3 className="section-header">Identity</h3>
          <div className="fingerprint-box" id="settings-fingerprint">
            <span className="fingerprint-label">Your Identity Fingerprint</span>
            <span className="fingerprint-value-row">
              {identity?.fingerprint}
              <button onClick={() => { if (identity?.fingerprint) { navigator.clipboard.writeText(identity.fingerprint); setFpCopied(true); setTimeout(() => setFpCopied(false), 2000); } }}
                aria-label="Copy fingerprint" className="btn btn--ghost btn--icon-sm">
                {fpCopied ? <span className="copied-pop"><CheckIcon size={14} /></span> : <CopyIcon size={14} />}
              </button>
            </span>
          </div>
        </div>

        <div className="settings-footer">
          M2M Secure Messenger v0.1.0 — End-to-End Encrypted
        </div>
      </div>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
