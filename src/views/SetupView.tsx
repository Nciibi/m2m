import { ToastContainer } from "../components/ui";
import type { Toast as ToastType } from "../types";

interface Props {
  toasts: ToastType[];
  removeToast: (id: string) => void;
}

/**
 * Splash screen shown while Tauri initializes the secure enclave.
 * Premium animated lock icon with depth and pulse ring.
 */
export default function SetupView({ toasts, removeToast }: Props) {
  return (
    <div className="app-container">
      <div className="centered-view">
        {/* Premium animated lock icon */}
        <div
          className="setup-icon"
          style={{
            width: 80,
            height: 80,
            borderRadius: "var(--radius-xl)",
            fontSize: "2.2rem",
            background: "var(--color-accent-gradient)",
            border: "1px solid rgba(255,255,255,0.15)",
            boxShadow: "var(--shadow-accent-strong), 0 0 60px rgba(99,102,241,0.15)",
            position: "relative",
          }}
        >
          <span style={{ filter: "drop-shadow(0 2px 4px rgba(0,0,0,0.3))" }}>
            🔑
          </span>
          {/* Outer glow ring */}
          <div
            style={{
              position: "absolute",
              inset: -8,
              borderRadius: "var(--radius-2xl)",
              border: "2px solid rgba(99,102,241,0.15)",
              animation: "pulseRing 2.5s ease-in-out infinite",
              pointerEvents: "none",
            }}
          />
        </div>

        <h2 style={{ marginTop: "var(--space-xl)" }}>
          Initializing Secure Enclave
        </h2>

        <p
          style={{
            maxWidth: 380,
            textAlign: "center",
            lineHeight: 1.7,
            margin: "var(--space-xs) 0 var(--space-lg)",
            color: "var(--color-text-secondary)",
          }}
        >
          Generating Ed25519 identity keys.
          <br />
          They never leave your device.
        </p>

        {/* Premium loading dots */}
        <div className="loading-dots" role="status" aria-label="Generating keys">
          <span />
          <span />
          <span />
        </div>

        {/* Crypto stack badge */}
        <div
          style={{
            marginTop: "var(--space-2xl)",
            padding: "var(--space-xs) var(--space-lg)",
            background: "var(--color-bg-card)",
            borderRadius: "var(--radius-full)",
            border: "1px solid var(--color-border-default)",
            fontSize: "var(--text-xs)",
            color: "var(--color-text-muted)",
            fontFamily: "var(--font-mono)",
            letterSpacing: "0.02em",
          }}
        >
          Ed25519 · X25519 · XChaCha20-Poly1305
        </div>
      </div>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
