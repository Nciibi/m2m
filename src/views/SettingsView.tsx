import { useState } from "react";
import {
  Button,
  Input,
  Badge,
  ToastContainer,
} from "../components/ui";
import type {
  Toast,
  IdentityInfo,
  NetworkSettings,
  StunConfig,
  NatTypeInfo,
} from "../types";

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
  identity,
  networkSettings,
  publicIp,
  stunLoading,
  networkDiagnostics,
  stunConfig,
  stunServerInput,
  privateMode,
  connectivityResult,
  toasts,
  removeToast,
  onBackToHub,
  onStunDiscover,
  onAddStunServer,
  onRemoveStunServer,
  onResetStunDefaults,
  onPrivateModeToggle,
  onConnectivityCheck,
  onTorToggle,
  setStunServerInput,
}: Props) {
  const [ipCopied, setIpCopied] = useState(false);
  const [fpCopied, setFpCopied] = useState(false);

  return (
    <div className="app-container">
      <div className="header">
        <h1>
          <span>⚙️</span> Settings
        </h1>
        <Button variant="secondary" compact onClick={onBackToHub} id="back-to-hub-btn">
          ← Back
        </Button>
      </div>

      <div className="content-area settings-content">
        {/* ═══ Public IP & Connectivity ═══ */}
        <div className="settings-section">
          <h3>Public IP & Connectivity</h3>

          <div className="settings-row">
            <div className="settings-label">
              <strong>Public Address</strong>
              <span className="settings-desc">
                Discovered via STUN — needed for invites that work across the internet.
                Queries all configured STUN servers in parallel for consensus.
              </span>
            </div>
            <div className="settings-value">
              {publicIp ? (
                <span className="mono-value" id="public-ip-display">
                  {publicIp}
                  <button
                    onClick={() => {
                      navigator.clipboard.writeText(publicIp);
                      setIpCopied(true);
                      setTimeout(() => setIpCopied(false), 2000);
                    }}
                    style={{
                      background: "none",
                      border: "none",
                      color: "inherit",
                      cursor: "pointer",
                      marginLeft: 8,
                      fontFamily: "inherit",
                      fontSize: "0.85rem",
                    }}
                    aria-label="Copy IP address"
                  >
                    {ipCopied ? "✅" : "📋"}
                  </button>
                </span>
              ) : (
                <span className="text-muted">Not discovered</span>
              )}
              <Button
                variant="secondary"
                compact
                onClick={onStunDiscover}
                disabled={stunLoading}
                id="stun-discover-btn"
                loading={stunLoading}
              >
                STUN Discover
              </Button>
            </div>
          </div>

          <div className="settings-row">
            <div className="settings-label">
              <strong>Connectivity Check</strong>
              <span className="settings-desc">
                Verify your listening port is reachable from the internet.
              </span>
            </div>
            <div className="settings-value">
              <Button
                variant="secondary"
                compact
                onClick={onConnectivityCheck}
                id="connectivity-check-btn"
              >
                Check Connectivity
              </Button>
            </div>
          </div>

          {connectivityResult && (
            <div
              className={`connectivity-result ${
                connectivityResult.reachable ? "success" : "warning"
              }`}
            >
              <strong>
                {connectivityResult.reachable
                  ? "✅ Reachable"
                  : "⚠️ Limited Reachability"}
              </strong>
              <div className="connectivity-details">
                <div>
                  NAT Type: <code>{connectivityResult.nat_type}</code>
                </div>
                {connectivityResult.behind_symmetric_nat && (
                  <div className="nat-warning">
                    ⚠️ Symmetric NAT detected — inbound connections may fail
                    without a TURN relay.
                  </div>
                )}
                {connectivityResult.public_addr && (
                  <div>
                    Public IP: <code>{connectivityResult.public_addr}</code>
                  </div>
                )}
                <div>
                  Local IPs:{" "}
                  {connectivityResult.host_addrs?.join(", ") || "none"}
                </div>
              </div>
            </div>
          )}
        </div>

        {/* ═══ STUN Servers ═══ */}
        <div className="settings-section" id="stun-servers-section">
          <h3>STUN Servers</h3>
          <p className="settings-desc">
            STUN servers are used to discover your public IP address. Configure
            multiple servers for redundancy and cross-verification.
          </p>
          {stunConfig && (
            <>
              <div className="stun-server-list">
                {stunConfig.servers.map((s, i) => (
                  <div key={i} className="stun-server-item">
                    <span className="mono-value">{s}</span>
                    <Button
                      variant="icon"
                      compact
                      onClick={() => onRemoveStunServer(i)}
                      aria-label={`Remove STUN server ${s}`}
                      style={{ padding: "2px 6px", minWidth: "auto", fontSize: "var(--text-sm)" }}
                    >
                      ✕
                    </Button>
                  </div>
                ))}
              </div>
              <div className="stun-server-add">
                <Input
                  placeholder="host:port (e.g., stun.example.com:3478)"
                  value={stunServerInput}
                  onChange={(e) => setStunServerInput(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && onAddStunServer()}
                  id="stun-server-input"
                  mono
                  compact
                />
                <Button variant="secondary" compact onClick={onAddStunServer} id="add-stun-server-btn">
                  Add
                </Button>
                <Button variant="secondary" compact onClick={onResetStunDefaults} id="reset-stun-btn">
                  Reset Defaults
                </Button>
              </div>
            </>
          )}
        </div>

        {/* ═══ Network Diagnostics ═══ */}
        {networkDiagnostics && (
          <div className="settings-section" id="network-diagnostics-section">
            <h3>Network Diagnostics</h3>
            <div className="diagnostics-grid">
              <div className="diagnostic-item">
                <span className="diagnostic-label">NAT Type</span>
                <span className="diagnostic-value">
                  <Badge
                    variant={
                      networkDiagnostics.nat_type === "symmetric" ? "warning"
                      : networkDiagnostics.nat_type === "blocked" ? "danger"
                      : networkDiagnostics.nat_type === "unknown" ? "default"
                      : "success"
                    }
                    compact
                  >
                    {networkDiagnostics.nat_type}
                  </Badge>
                </span>
              </div>
              <div className="diagnostic-item">
                <span className="diagnostic-label">Candidates</span>
                <span className="diagnostic-value">
                  {networkDiagnostics.candidates?.length || 0}
                </span>
              </div>
              <div className="diagnostic-item full-width">
                <span className="diagnostic-label">STUN Servers</span>
                <span className="diagnostic-value">
                  <div className="stun-health-list">
                    {networkDiagnostics.stun_servers?.map(
                      (s: any, i: number) => (
                        <div
                          key={i}
                          className={`stun-health-item ${s.reachable ? "ok" : "fail"}`}
                        >
                          <span>{s.reachable ? "✅" : "❌"}</span>
                          <code>{s.server}</code>
                          {s.rtt_ms && (
                            <span className="rtt">{s.rtt_ms}ms</span>
                          )}
                        </div>
                      )
                    )}
                  </div>
                </span>
              </div>
              {networkDiagnostics.candidates &&
                networkDiagnostics.candidates.length > 0 && (
                  <div className="diagnostic-item full-width">
                    <span className="diagnostic-label">
                      All Candidates (sorted by priority)
                    </span>
                    <div className="candidate-list">
                      {networkDiagnostics.candidates.map(
                        (c: any, i: number) => (
                          <div
                            key={i}
                            className={`candidate-item type-${c.candidate_type}`}
                          >
                            <span className="candidate-type">
                              {c.candidate_type === 0
                                ? "🏠 Host"
                                : c.candidate_type === 1
                                  ? "🌐 SRFLX"
                                  : c.candidate_type === 2
                                    ? "🔄 PRFLX"
                                    : "🔄 Relay"}
                            </span>
                            <code>{c.address}</code>
                            <span className="candidate-priority">
                              prio: {c.priority}
                            </span>
                            {i === 0 && (
                              <span className="candidate-active">← Active</span>
                            )}
                          </div>
                        )
                      )}
                    </div>
                  </div>
                )}
            </div>
          </div>
        )}

        {/* ═══ Privacy ═══ */}
        <div className="settings-section">
          <h3>Privacy</h3>
          <div className="settings-row">
            <div className="settings-label">
              <strong>Private Mode</strong>
              <span className="settings-desc">
                When enabled, your public IP will NOT be included in invite links.
                Only local network addresses will be shared.
              </span>
            </div>
            <div className="settings-value">
              <Badge variant={privateMode ? "success" : "default"} compact dot>
                {privateMode ? "Enabled" : "Disabled"}
              </Badge>
              <Button
                variant={privateMode ? "danger" : "secondary"}
                compact
                onClick={onPrivateModeToggle}
                id="private-mode-toggle"
              >
                {privateMode ? "Disable Private Mode" : "Enable Private Mode"}
              </Button>
            </div>
          </div>
        </div>

        {/* ═══ Tor Routing ═══ */}
        <div className="settings-section">
          <h3>Tor Routing</h3>
          <div className="settings-row">
            <div className="settings-label">
              <strong>Tor Routing</strong>
              <span className="settings-desc">
                Route all outgoing connections through Tor SOCKS5 proxy
                (127.0.0.1:9050).
              </span>
            </div>
            <div className="settings-value">
              <Badge
                variant={networkSettings?.tor_reachable ? "success" : "default"}
                compact
                dot={!!networkSettings?.tor_reachable}
              >
                {networkSettings?.tor_reachable
                  ? "Proxy reachable"
                  : "Proxy not found"}
              </Badge>
              <Button
                variant={networkSettings?.tor_enabled ? "danger" : "secondary"}
                compact
                onClick={onTorToggle}
                id="tor-toggle-btn"
              >
                {networkSettings?.tor_enabled ? "Disable Tor" : "Enable Tor"}
              </Button>
            </div>
          </div>
        </div>

        {/* ═══ Identity ═══ */}
        <div className="settings-section">
          <h3>Identity</h3>
          <div className="fingerprint-box" id="settings-fingerprint">
            <span className="fingerprint-label">Your Identity Fingerprint</span>
            <span
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                gap: 8,
              }}
            >
              {identity?.fingerprint}
              <Button
                variant="ghost"
                compact
                onClick={() => {
                  if (identity?.fingerprint) {
                    navigator.clipboard.writeText(identity.fingerprint);
                    setFpCopied(true);
                    setTimeout(() => setFpCopied(false), 2000);
                  }
                }}
                aria-label="Copy fingerprint"
                style={{ fontSize: "var(--text-sm)", padding: "2px 6px" }}
              >
                {fpCopied ? "✅" : "📋"}
              </Button>
            </span>
          </div>
        </div>

        {/* Version */}
        <div className="settings-version">
          M2M Secure Messenger v0.1.0 — End-to-End Encrypted
        </div>
      </div>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
