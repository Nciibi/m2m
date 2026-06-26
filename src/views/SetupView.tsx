import { ToastContainer } from "../toast";
import type { Toast } from "../types";

interface Props {
  toasts: Toast[];
  removeToast: (id: string) => void;
}

/** Loading screen shown while Tauri initializes the secure enclave. */
export default function SetupView({ toasts, removeToast }: Props) {
  return (
    <div className="app-container">
      <div className="centered-view">
        <div className="setup-icon">🔑</div>
        <h2>Initializing Secure Enclave</h2>
        <p>
          Generating Ed25519 identity keys.
          <br />
          They never leave your device.
        </p>
        <div className="loading-dots">
          <span />
          <span />
          <span />
        </div>
      </div>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
