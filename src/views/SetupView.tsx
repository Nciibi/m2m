import { ToastContainer } from "../components/ui";
import type { Toast as ToastType } from "../types";

interface Props {
  toasts: ToastType[];
  removeToast: (id: string) => void;
}

/**
 * Splash screen shown while Tauri initializes the secure enclave.
 * Animated lock icon with loading dots.
 */
export default function SetupView({ toasts, removeToast }: Props) {
  return (
    <div className="app-container">
      <div className="centered-view">
        {/* Animated lock icon */}
        <div
          className="setup-icon"
          style={{
            animation: "pulseRing 2s ease-in-out infinite",
          }}
          aria-hidden="true"
        >
          🔑
        </div>

        <h2>Initializing Secure Enclave</h2>

        <p
          style={{
            maxWidth: 380,
            textAlign: "center",
            lineHeight: 1.7,
            margin: "8px 0 20px",
          }}
        >
          Generating Ed25519 identity keys.
          <br />
          They never leave your device.
        </p>

        {/* Loading dots */}
        <div
          className="loading-dots"
          role="status"
          aria-label="Generating keys"
        >
          <span />
          <span />
          <span />
        </div>

        <p
          style={{
            marginTop: 24,
            fontSize: "var(--text-sm)",
            color: "var(--color-text-muted)",
          }}
        >
          Ed25519 • X25519 • XChaCha20-Poly1305
        </p>
      </div>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
