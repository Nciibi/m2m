import { useState } from "react";
import { Button, Input, Badge, ToastContainer } from "../components/ui";
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
          <span
            style={{
              display: "inline-flex",
              width: 32,
              height: 32,
              borderRadius: "var(--radius-sm)",
              background: "var(--color-bg-input)",
              border: "1px solid var(--color-border-default)",
              alignItems: "center",
              justifyContent: "center",
              fontSize: "1rem",
            }}
          >
            ⚙️
          </span>
          Settings
        </h1>
        <Button
          variant="secondary"
          compact
          onClick={onBackToHub}
          id="back-to-hub-btn"
        >
          <span>←</span> Back
        </Button>
      </div>

      <div
        className="content-area"
        style={{
          padding: "var(--space-xl) var(--space-2xl)",
          overflowY: "auto",
          scrollbarWidth: "thin",
          scrollbarColor: "var(--scrollbar-thumb) transparent",
        }}
      >
        {/* ═══ Public IP & Connectivity ═══ */}
        <div style={{ marginBottom: "var(--space-2xl)" }}>
          <h3 className="section-header">Public IP & Connectivity</h3>

          <div
            className="settings-row"
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "var(--space-md) var(--space-lg)",
              background: "var(--color-bg-card)",
              border: "1px solid var(--color-border-default)",
              borderRadius: "var(--radius-md)",
              marginBottom: "var(--space-xs)",
              gap: "var(--space-md)",
            }}
          >
            <div
              className="settings-label"
              style={{
                display: "flex",
                flexDirection: "column",
                gap: "var(--space-xxs)",
                flex: 1,
              }}
            >
              <strong
                style={{
                  fontSize: "var(--text-md)",
                  fontWeight: 500,
                  color: "var(--color-text-primary)",
                }}
              >
                Public Address
              </strong>
              <span
                className="settings-desc"
                style={{
                  fontSize: "var(--text-sm)",
                  color: "var(--color-text-muted)",
                  lineHeight: 1.4,
                }}
              >
                Discovered via STUN — needed for invites across the internet.
              </span>
            </div>
            <div
              className="settings-value"
              style={{
                display: "flex",
                alignItems: "center",
                gap: "var(--space-sm)",
                flexShrink: 0,
              }}
            >
              {publicIp ? (
                <span
                  className="mono-value"
                  id="public-ip-display"
                  style={{
                    fontFamily: "var(--font-mono)",
                    fontSize: "var(--text-base)",
                    color: "var(--color-text-accent)",
                    background: "rgba(0,0,0,0.2)",
                    padding: "var(--space-xxs) var(--space-sm)",
                    borderRadius: "var(--radius-xs)",
                    display: "inline-flex",
                    alignItems: "center",
                    gap: 6,
                  }}
                >
                  {publicIp}
                  <button
                    onClick={() => {
                      navigator.clipboard.writeText(publicIp);
                      setIpCopied(true);
                      setTimeout(() => setIpCopied(false), 2000);
                    }}
                    aria-label="Copy IP address"
                    style={{
                      background: "none",
                      border: "none",
                      color: "var(--color-text-muted)",
                      cursor: "pointer",
                      fontSize: "0.85rem",
                      fontFamily: "inherit",
                      padding: 0,
                    }}
                  >
                    {ipCopied ? "✅" : "📋"}
                  </button>
                </span>
              ) : (
                <span style={{ color: "var(--color-text-muted)", fontSize: "var(--text-sm)" }}>
                  Not discovered
                </span>
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

          <div
            className="settings-row"
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "var(--space-md) var(--space-lg)",
              background: "var(--color-bg-card)",
              border: "1px solid var(--color-border-default)",
              borderRadius: "var(--radius-md)",
              marginBottom: "var(--space-xs)",
              gap: "var(--space-md)",
            }}
          >
            <div
              className="settings-label"
              style={{
                display: "flex",
                flexDirection: "column",
                gap: "var(--space-xxs)",
                flex: 1,
              }}
            >
              <strong
                style={{
                  fontSize: "var(--text-md)",
                  fontWeight: 500,
                  color: "var(--color-text-primary)",
                }}
              >
                Connectivity Check
              </strong>
              <span
                className="settings-desc"
                style={{
                  fontSize: "var(--text-sm)",
                  color: "var(--color-text-muted)",
                  lineHeight: 1.4,
                }}
              >
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
                Check
              </Button>
            </div>
          </div>

          {connectivityResult && (
            <div
              className="connectivity-result"
              style={{
                marginTop: "var(--space-sm)",
                padding: "var(--space-md) var(--space-lg)",
                borderRadius: "var(--radius-md)",
                fontSize: "var(--text-sm)",
                lineHeight: 1.6,
                animation: "msgSlide 300ms var(--ease-out-expo)",
                background: connectivityResult.reachable
                  ? "var(--color-success-bg)"
                  : "var(--color-warning-bg)",
                border: `1px solid ${
                  connectivityResult.reachable
                    ? "rgba(16,185,129,0.2)"
                    : "rgba(245,158,11,0.2)"
                }`,
              }}
            >
              <strong
                style={{
                  color: connectivityResult.reachable
                    ? "var(--color-success)"
                    : "var(--color-warning)",
                }}
              >
                {connectivityResult.reachable
                  ? "✅ Reachable"
                  : "⚠️ Limited Reachability"}
              </strong>
              <div
                style={{
                  marginTop: "var(--space-xs)",
                  fontSize: "var(--text-sm)",
                  color: "var(--color-text-secondary)",
                }}
              >
                <div>
                  NAT Type:{" "}
                  <code
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: "var(--text-sm)",
                      background: "rgba(0,0,0,0.15)",
                      padding: "1px 6px",
                      borderRadius: 3,
                    }}
                  >
                    {connectivityResult.nat_type}
                  </code>
                </div>
                {connectivityResult.behind_symmetric_nat && (
                  <div
                    className="nat-warning"
                    style={{
                      marginTop: "var(--space-xxs)",
                      padding: "var(--space-xxs) var(--space-sm)",
                      background: "var(--color-warning-bg)",
                      borderRadius: "var(--radius-xs)",
                      color: "var(--color-warning)",
                      fontWeight: 500,
                      fontSize: "var(--text-sm)",
                    }}
                  >
                    ⚠️ Symmetric NAT detected — inbound connections may fail
                    without a TURN relay.
                  </div>
                )}
                {connectivityResult.public_addr && (
                  <div>
                    Public IP:{" "}
                    <code
                      style={{
                        fontFamily: "var(--font-mono)",
                        fontSize: "var(--text-sm)",
                        background: "rgba(0,0,0,0.15)",
                        padding: "1px 6px",
                        borderRadius: 3,
                      }}
                    >
                      {connectivityResult.public_addr}
                    </code>
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
        <div style={{ marginBottom: "var(--space-2xl)" }} id="stun-servers-section">
          <h3 className="section-header">STUN Servers</h3>
          <p
            className="settings-desc"
            style={{
              display: "block",
              marginBottom: "var(--space-sm)",
              fontSize: "var(--text-sm)",
              color: "var(--color-text-muted)",
              lineHeight: 1.5,
            }}
          >
            STUN servers discover your public IP. Configure multiple servers
            for redundancy and cross-verification.
          </p>
          {stunConfig && (
            <>
              <div
                className="stun-server-list"
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: "var(--space-xxs)",
                  marginBottom: "var(--space-sm)",
                }}
              >
                {stunConfig.servers.map((s, i) => (
                  <div
                    key={i}
                    className="stun-server-item"
                    style={{
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "center",
                      padding: "var(--space-xs) var(--space-md)",
                      background: "rgba(0,0,0,0.12)",
                      border: "1px solid var(--color-border-default)",
                      borderRadius: "var(--radius-sm)",
                    }}
                  >
                    <span
                      style={{
                        fontFamily: "var(--font-mono)",
                        fontSize: "var(--text-sm)",
                        color: "var(--color-text-accent)",
                      }}
                    >
                      {s}
                    </span>
                    <button
                      onClick={() => onRemoveStunServer(i)}
                      aria-label={`Remove STUN server ${s}`}
                      style={{
                        padding: "2px 8px",
                        fontSize: "var(--text-sm)",
                        background: "transparent",
                        border: "1px solid var(--color-border-default)",
                        color: "var(--color-text-muted)",
                        borderRadius: "var(--radius-xs)",
                        cursor: "pointer",
                        fontFamily: "inherit",
                        transition: "var(--transition-fast)",
                      }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.color = "var(--color-danger)";
                        e.currentTarget.style.borderColor =
                          "rgba(239,68,68,0.3)";
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.color =
                          "var(--color-text-muted)";
                        e.currentTarget.style.borderColor =
                          "var(--color-border-default)";
                      }}
                    >
                      ✕
                    </button>
                  </div>
                ))}
              </div>
              <div
                className="stun-server-add"
                style={{
                  display: "flex",
                  gap: "var(--space-xs)",
                  alignItems: "center",
                  flexWrap: "wrap",
                }}
              >
                <Input
                  placeholder="host:port (e.g., stun.example.com:3478)"
                  value={stunServerInput}
                  onChange={(e) => setStunServerInput(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && onAddStunServer()}
                  id="stun-server-input"
                  mono
                  compact
                />
                <Button
                  variant="secondary"
                  compact
                  onClick={onAddStunServer}
                  id="add-stun-server-btn"
                >
                  Add
                </Button>
                <Button
                  variant="secondary"
                  compact
                  onClick={onResetStunDefaults}
                  id="reset-stun-btn"
                >
                  Reset Defaults
                </Button>
              </div>
            </>
          )}
        </div>

        {/* ═══ Network Diagnostics ═══ */}
        {networkDiagnostics && (
          <div style={{ marginBottom: "var(--space-2xl)" }} id="network-diagnostics-section">
            <h3 className="section-header">Network Diagnostics</h3>
            <div
              className="diagnostics-grid"
              style={{
                display: "grid",
                gridTemplateColumns: "1fr 1fr",
                gap: "var(--space-xs)",
              }}
            >
              <div
                className="diagnostic-item"
                style={{
                  padding: "var(--space-sm) var(--space-md)",
                  background: "rgba(0,0,0,0.12)",
                  border: "1px solid var(--color-border-default)",
                  borderRadius: "var(--radius-sm)",
                  display: "flex",
                  flexDirection: "column",
                  gap: "var(--space-xxs)",
                }}
              >
                <span
                  className="diagnostic-label"
                  style={{
                    fontSize: "var(--text-xs)",
                    textTransform: "uppercase",
                    letterSpacing: "0.08em",
                    color: "var(--color-text-muted)",
                    fontWeight: 600,
                  }}
                >
                  NAT Type
                </span>
                <span
                  className="diagnostic-value"
                  style={{ fontSize: "var(--text-sm)", color: "var(--color-text-primary)" }}
                >
                  <Badge
                    variant={
                      networkDiagnostics.nat_type === "symmetric"
                        ? "warning"
                        : networkDiagnostics.nat_type === "blocked"
                          ? "danger"
                          : networkDiagnostics.nat_type === "unknown"
                            ? "default"
                            : "success"
                    }
                    compact
                  >
                    {networkDiagnostics.nat_type}
                  </Badge>
                </span>
              </div>
              <div
                className="diagnostic-item"
                style={{
                  padding: "var(--space-sm) var(--space-md)",
                  background: "rgba(0,0,0,0.12)",
                  border: "1px solid var(--color-border-default)",
                  borderRadius: "var(--radius-sm)",
                  display: "flex",
                  flexDirection: "column",
                  gap: "var(--space-xxs)",
                }}
              >
                <span
                  className="diagnostic-label"
                  style={{
                    fontSize: "var(--text-xs)",
                    textTransform: "uppercase",
                    letterSpacing: "0.08em",
                    color: "var(--color-text-muted)",
                    fontWeight: 600,
                  }}
                >
                  Candidates
                </span>
                <span
                  className="diagnostic-value"
                  style={{ fontSize: "var(--text-sm)", color: "var(--color-text-primary)" }}
                >
                  {networkDiagnostics.candidates?.length || 0}
                </span>
              </div>
              <div
                className="diagnostic-item full-width"
                style={{
                  padding: "var(--space-sm) var(--space-md)",
                  background: "rgba(0,0,0,0.12)",
                  border: "1px solid var(--color-border-default)",
                  borderRadius: "var(--radius-sm)",
                  display: "flex",
                  flexDirection: "column",
                  gap: "var(--space-xxs)",
                  gridColumn: "span 2",
                }}
              >
                <span
                  className="diagnostic-label"
                  style={{
                    fontSize: "var(--text-xs)",
                    textTransform: "uppercase",
                    letterSpacing: "0.08em",
                    color: "var(--color-text-muted)",
                    fontWeight: 600,
                  }}
                >
                  STUN Servers
                </span>
                <span className="diagnostic-value">
                  <div
                    className="stun-health-list"
                    style={{
                      display: "flex",
                      flexDirection: "column",
                      gap: "var(--space-xxs)",
                    }}
                  >
                    {networkDiagnostics.stun_servers?.map(
                      (s: any, i: number) => (
                        <div
                          key={i}
                          className={`stun-health-item ${s.reachable ? "ok" : "fail"}`}
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: "var(--space-xxs)",
                            fontSize: "var(--text-sm)",
                            padding: "var(--space-xxs) var(--space-sm)",
                            borderRadius: "var(--radius-xs)",
                            background: "rgba(0,0,0,0.08)",
                            borderLeft: `3px solid ${
                              s.reachable
                                ? "var(--color-success)"
                                : "var(--color-danger)"
                            }`,
                          }}
                        >
                          <span>
                            {s.reachable ? "✅" : "❌"}
                          </span>
                          <code
                            style={{
                              flex: 1,
                              fontSize: "var(--text-xs)",
                              fontFamily: "var(--font-mono)",
                            }}
                          >
                            {s.server}
                          </code>
                          {s.rtt_ms && (
                            <span
                              style={{
                                color: "var(--color-text-muted)",
                                fontSize: "var(--text-xs)",
                              }}
                            >
                              {s.rtt_ms}ms
                            </span>
                          )}
                        </div>
                      )
                    )}
                  </div>
                </span>
              </div>
              {networkDiagnostics.candidates &&
                networkDiagnostics.candidates.length > 0 && (
                  <div
                    className="diagnostic-item full-width"
                    style={{
                      padding: "var(--space-sm) var(--space-md)",
                      background: "rgba(0,0,0,0.12)",
                      border: "1px solid var(--color-border-default)",
                      borderRadius: "var(--radius-sm)",
                      display: "flex",
                      flexDirection: "column",
                      gap: "var(--space-xxs)",
                      gridColumn: "span 2",
                    }}
                  >
                    <span
                      className="diagnostic-label"
                      style={{
                        fontSize: "var(--text-xs)",
                        textTransform: "uppercase",
                        letterSpacing: "0.08em",
                        color: "var(--color-text-muted)",
                        fontWeight: 600,
                      }}
                    >
                      All Candidates (sorted by priority)
                    </span>
                    <div
                      className="candidate-list"
                      style={{
                        display: "flex",
                        flexDirection: "column",
                        gap: "var(--space-xxs)",
                      }}
                    >
                      {networkDiagnostics.candidates.map(
                        (c: any, i: number) => (
                          <div
                            key={i}
                            className={`candidate-item type-${c.candidate_type}`}
                            style={{
                              display: "flex",
                              alignItems: "center",
                              gap: "var(--space-xs)",
                              padding: "var(--space-xxs) var(--space-sm)",
                              borderRadius: "var(--radius-xs)",
                              fontSize: "var(--text-sm)",
                              background: "rgba(0,0,0,0.08)",
                              borderLeft: `3px solid ${
                                c.candidate_type === 0
                                  ? "var(--color-accent)"
                                  : c.candidate_type === 1
                                    ? "var(--color-success)"
                                    : c.candidate_type === 2
                                      ? "var(--color-warning)"
                                      : "var(--color-danger)"
                              }`,
                            }}
                          >
                            <span
                              className="candidate-type"
                              style={{
                                fontWeight: 500,
                                minWidth: 60,
                                fontSize: "var(--text-xs)",
                              }}
                            >
                              {c.candidate_type === 0
                                ? "🏠 Host"
                                : c.candidate_type === 1
                                  ? "🌐 SRFLX"
                                  : c.candidate_type === 2
                                    ? "🔄 PRFLX"
                                    : "🔄 Relay"}
                            </span>
                            <code
                              style={{
                                flex: 1,
                                fontSize: "var(--text-sm)",
                                fontFamily: "var(--font-mono)",
                              }}
                            >
                              {c.address}
                            </code>
                            <span
                              className="candidate-priority"
                              style={{
                                color: "var(--color-text-muted)",
                                fontSize: "var(--text-xs)",
                              }}
                            >
                              prio: {c.priority}
                            </span>
                            {i === 0 && (
                              <span
                                className="candidate-active"
                                style={{
                                  fontSize: "var(--text-xs)",
                                  fontWeight: 600,
                                  color: "var(--color-success)",
                                  background: "var(--color-success-bg)",
                                  padding: "1px 8px",
                                  borderRadius: "var(--radius-full)",
                                }}
                              >
                                Active
                              </span>
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
        <div style={{ marginBottom: "var(--space-2xl)" }}>
          <h3 className="section-header">Privacy</h3>
          <div
            className="settings-row"
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "var(--space-md) var(--space-lg)",
              background: "var(--color-bg-card)",
              border: "1px solid var(--color-border-default)",
              borderRadius: "var(--radius-md)",
              gap: "var(--space-md)",
            }}
          >
            <div
              className="settings-label"
              style={{
                display: "flex",
                flexDirection: "column",
                gap: "var(--space-xxs)",
                flex: 1,
              }}
            >
              <strong
                style={{
                  fontSize: "var(--text-md)",
                  fontWeight: 500,
                  color: "var(--color-text-primary)",
                }}
              >
                Private Mode
              </strong>
              <span
                className="settings-desc"
                style={{
                  fontSize: "var(--text-sm)",
                  color: "var(--color-text-muted)",
                  lineHeight: 1.4,
                }}
              >
                When enabled, your public IP will NOT be included in invite
                links. Only local network addresses will be shared.
              </span>
            </div>
            <div
              className="settings-value"
              style={{
                display: "flex",
                alignItems: "center",
                gap: "var(--space-sm)",
                flexShrink: 0,
              }}
            >
              <Badge
                variant={privateMode ? "success" : "default"}
                compact
                dot
              >
                {privateMode ? "Enabled" : "Disabled"}
              </Badge>
              <Button
                variant={privateMode ? "danger" : "secondary"}
                compact
                onClick={onPrivateModeToggle}
                id="private-mode-toggle"
              >
                {privateMode ? "Disable" : "Enable"}
              </Button>
            </div>
          </div>
        </div>

        {/* ═══ Tor Routing ═══ */}
        <div style={{ marginBottom: "var(--space-2xl)" }}>
          <h3 className="section-header">Tor Routing</h3>
          <div
            className="settings-row"
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "var(--space-md) var(--space-lg)",
              background: "var(--color-bg-card)",
              border: "1px solid var(--color-border-default)",
              borderRadius: "var(--radius-md)",
              gap: "var(--space-md)",
            }}
          >
            <div
              className="settings-label"
              style={{
                display: "flex",
                flexDirection: "column",
                gap: "var(--space-xxs)",
                flex: 1,
              }}
            >
              <strong
                style={{
                  fontSize: "var(--text-md)",
                  fontWeight: 500,
                  color: "var(--color-text-primary)",
                }}
              >
                Tor Routing
              </strong>
              <span
                className="settings-desc"
                style={{
                  fontSize: "var(--text-sm)",
                  color: "var(--color-text-muted)",
                  lineHeight: 1.4,
                }}
              >
                Route all outgoing connections through Tor SOCKS5 proxy
                (127.0.0.1:9050).
              </span>
            </div>
            <div
              className="settings-value"
              style={{
                display: "flex",
                alignItems: "center",
                gap: "var(--space-sm)",
                flexShrink: 0,
              }}
            >
              <Badge
                variant={
                  networkSettings?.tor_reachable ? "success" : "default"
                }
                compact
                dot={!!networkSettings?.tor_reachable}
              >
                {networkSettings?.tor_reachable
                  ? "Proxy reachable"
                  : "Proxy not found"}
              </Badge>
              <Button
                variant={
                  networkSettings?.tor_enabled ? "danger" : "secondary"
                }
                compact
                onClick={onTorToggle}
                id="tor-toggle-btn"
              >
                {networkSettings?.tor_enabled
                  ? "Disable Tor"
                  : "Enable Tor"}
              </Button>
            </div>
          </div>
        </div>

        {/* ═══ Identity ═══ */}
        <div style={{ marginBottom: "var(--space-2xl)" }}>
          <h3 className="section-header">Identity</h3>
          <div
            className="fingerprint-box"
            id="settings-fingerprint"
            style={{
              background: "rgba(0,0,0,0.2)",
              padding: "var(--space-md) var(--space-lg)",
              borderRadius: "var(--radius-md)",
              fontFamily: "var(--font-mono)",
              fontSize: "var(--text-base)",
              color: "var(--color-text-accent)",
              letterSpacing: "0.5px",
              wordBreak: "break-all",
              border: "1px solid var(--color-border-default)",
              textAlign: "center",
              position: "relative",
            }}
          >
            <span className="fingerprint-label" style={{
              display: "block",
              color: "var(--color-text-muted)",
              fontFamily: "var(--font-sans)",
              fontSize: "var(--text-xs)",
              textTransform: "uppercase",
              letterSpacing: "0.08em",
              marginBottom: "var(--space-sm)",
              fontWeight: 500,
            }}>
              Your Identity Fingerprint
            </span>
            <span
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                gap: "var(--space-xs)",
                flexWrap: "wrap",
              }}
            >
              {identity?.fingerprint}
              <button
                onClick={() => {
                  if (identity?.fingerprint) {
                    navigator.clipboard.writeText(identity.fingerprint);
                    setFpCopied(true);
                    setTimeout(() => setFpCopied(false), 2000);
                  }
                }}
                aria-label="Copy fingerprint"
                style={{
                  fontSize: "var(--text-sm)",
                  padding: "2px 8px",
                  background: "transparent",
                  border: "1px solid var(--color-border-default)",
                  borderRadius: "var(--radius-xs)",
                  color: "var(--color-text-muted)",
                  cursor: "pointer",
                  fontFamily: "inherit",
                  transition: "var(--transition-fast)",
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.color = "var(--color-text-secondary)";
                  e.currentTarget.style.borderColor = "var(--color-border-strong)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.color = "var(--color-text-muted)";
                  e.currentTarget.style.borderColor = "var(--color-border-default)";
                }}
              >
                {fpCopied ? "✅" : "📋"}
              </button>
            </span>
          </div>
        </div>

        {/* Version */}
        <div
          className="settings-version"
          style={{
            textAlign: "center",
            padding: "var(--space-lg)",
            fontSize: "var(--text-xs)",
            color: "var(--color-text-muted)",
            borderTop: "1px solid var(--color-border-default)",
          }}
        >
          M2M Secure Messenger v0.1.0 — End-to-End Encrypted
        </div>
      </div>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
