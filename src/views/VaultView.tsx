import { useState, useEffect } from "react";
import { ToastContainer } from "../toast";
import { estimateEntropy } from "../utils";
import type { Toast } from "../types";

interface Props {
  vaultInitialized: boolean;
  onError?: (msg: string) => void;
  onUnlock: (passphrase: string) => Promise<void>;
  toasts: Toast[];
  removeToast: (id: string) => void;
}

export default function VaultView({
  vaultInitialized,
  onUnlock,
  toasts,
  removeToast,
}: Props) {
  const [passphrase, setPassphrase] = useState("");
  const [passphraseConfirm, setPassphraseConfirm] = useState("");
  const [vaultError, setVaultError] = useState("");
  const [passphraseStrength, setPassphraseStrength] = useState({
    percent: 0,
    label: "",
    class: "",
  });

  const isFirstTime = !vaultInitialized;

  useEffect(() => {
    const entropy = estimateEntropy(passphrase);
    let percent: number;
    let label: string;
    let cls: string;
    if (passphrase.length === 0) {
      percent = 0;
      label = "";
      cls = "";
    } else if (passphrase.length < 12) {
      percent = Math.min(30, passphrase.length * 5);
      label = "Too short";
      cls = "weak";
    } else if (entropy < 40) {
      percent = 40;
      label = "Weak";
      cls = "weak";
    } else if (entropy < 60) {
      percent = 65;
      label = "Fair";
      cls = "fair";
    } else if (entropy < 80) {
      percent = 85;
      label = "Strong";
      cls = "strong";
    } else {
      percent = 100;
      label = "Very Strong";
      cls = "very-strong";
    }
    setPassphraseStrength({ percent, label, class: cls });
  }, [passphrase]);

  const handleUnlock = async () => {
    setVaultError("");
    if (passphrase.length < 12) {
      setVaultError("Passphrase must be at least 12 characters.");
      return;
    }
    if (!vaultInitialized && passphraseConfirm && passphrase !== passphraseConfirm) {
      setVaultError("Passphrases do not match.");
      return;
    }
    if (!vaultInitialized && !passphraseConfirm) {
      setVaultError("Please confirm your passphrase.");
      return;
    }
    const est = estimateEntropy(passphrase);
    if (est < 40) {
      setVaultError(
        `Passphrase too weak: ~${Math.round(est)} bits. Use a longer passphrase (aim for 60+).`
      );
      return;
    }
    try {
      await onUnlock(passphrase);
    } catch (e: any) {
      setVaultError(String(e));
    }
  };

  return (
    <div className="app-container">
      <div className="centered-view">
        <div className="setup-icon vault-icon">🔐</div>
        <h2>{isFirstTime ? "Set Up Your Vault" : "Unlock Your Vault"}</h2>
        <p>
          {isFirstTime
            ? "Choose a passphrase to encrypt your local data. This protects your identity keys and message history."
            : "Enter your passphrase to decrypt your local data."}
          <br />
          Minimum 12 characters. Uses Argon2id key derivation.
        </p>
        <div className="vault-form">
          <input
            id="vault-passphrase"
            type="password"
            placeholder="Passphrase"
            value={passphrase}
            onChange={(e) => setPassphrase(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleUnlock()}
          />
          {passphrase.length > 0 && (
            <div className="passphrase-strength">
              <div className="strength-bar">
                <div
                  className={`strength-fill ${passphraseStrength.class}`}
                  style={{ width: `${passphraseStrength.percent}%` }}
                />
              </div>
              <span className={`strength-label ${passphraseStrength.class}`}>
                {passphraseStrength.label}
                {passphraseStrength.label && " — "}
                {passphrase.length} chars
              </span>
            </div>
          )}
          {isFirstTime && (
            <input
              id="vault-passphrase-confirm"
              type="password"
              placeholder="Confirm passphrase"
              value={passphraseConfirm}
              onChange={(e) => setPassphraseConfirm(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleUnlock()}
            />
          )}
          {vaultError && <div className="vault-error">{vaultError}</div>}
          <button id="vault-unlock-btn" onClick={handleUnlock}>
            {isFirstTime ? "Create Vault" : "Unlock"}
          </button>
        </div>
      </div>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
