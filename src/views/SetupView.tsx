import { ToastContainer } from "../components/ui";
import { KeyIcon } from "../components/ui/Icons";
import type { Toast as ToastType } from "../types";

interface Props {
  toasts: ToastType[];
  removeToast: (id: string) => void;
}

export default function SetupView({ toasts, removeToast }: Props) {
  return (
    <div className="app-shell">
      <div className="centered-view">
        <div className="setup-icon">
          <KeyIcon size={36} color="white" />
          <div className="setup-icon__glow" />
        </div>

        <h2 className="centered-view__title" style={{ marginTop: "var(--space-xl)" }}>
          Initializing Secure Enclave
        </h2>

        <p className="centered-view__desc" style={{ margin: "var(--space-xs) 0 var(--space-lg)" }}>
          Generating Ed25519 identity keys.
          <br />
          They never leave your device.
        </p>

        <div className="loading-dots" role="status" aria-label="Generating keys">
          <span /><span /><span />
        </div>

        <div className="crypto-badge">
          Ed25519 · X25519 · XChaCha20-Poly1305
        </div>
      </div>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
