import { ToastContainer } from "../components/ui";
import type { ToastData } from "../components/ui/Toast";
import { KeyIcon } from "../components/ui/Icons";

interface Props {
  toasts: ToastData[];
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

        <h2 className="centered-view__title centered-view__title--spaced">
          Initializing Secure Enclave
        </h2>

        <p className="centered-view__desc centered-view__desc--spaced">
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
