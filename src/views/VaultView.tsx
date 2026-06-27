import { useState, useEffect } from "react";
import { Button, Input, ToastContainer } from "../components/ui";
import type { Toast as ToastType } from "../types";
import { estimateEntropy } from "../utils";

interface Props {
  vaultInitialized: boolean;
  onUnlock: (passphrase: string) => Promise<void>;
  toasts: ToastType[];
  removeToast: (id: string) => void;
}

/**
 * Vault unlock / first-time setup screen.
 * Shows passphrase strength meter, confirmation, and error feedback.
 */
export default function VaultView({
  vaultInitialized,
  onUnlock,
  toasts,
  removeToast,
}: Props) {
  const [passphrase, setPassphrase] = useState("");
  const [passphraseConfirm, setPassphraseConfirm] = useState("");
  const [vaultError, setVaultError] = useState("");
  const [showPassphrase, setShowPassphrase] = useState(false);
  const [loading, setLoading] = useState(false);
  const [showTips, setShowTips] = useState(false);
  const [passphraseStrength, setPassphraseStrength] = useState({
    percent: 0,
    bits: 0,
    label: "",
    class: "",
  });

  const isFirstTime = !vaultInitialized;

  // ─── Strength Meter ───
  useEffect(() => {
    const entropy = estimateEntropy(passphrase);
    let percent: number;
    let label: string;
    let cls: string;
    if (passphrase.length === 0) {
      percent = 0; label = ""; cls = "";
    } else if (passphrase.length < 12) {
      percent = Math.min(30, passphrase.length * 5);
      label = "Too short";
      cls = "weak";
    } else if (entropy < 40) {
      percent = 40; label = "Weak"; cls = "weak";
    } else if (entropy < 60) {
      percent = 65; label = "Fair"; cls = "fair";
    } else if (entropy < 80) {
      percent = 85; label = "Strong"; cls = "strong";
    } else {
      percent = 100; label = "Very Strong"; cls = "very-strong";
    }
    setPassphraseStrength({ percent, bits: Math.round(entropy), label, class: cls });
  }, [passphrase]);

  // ─── Submit ───
  const handleUnlock = async () => {
    setVaultError("");
    if (passphrase.length < 12) {
      setVaultError("Passphrase must be at least 12 characters.");
      return;
    }
    if (isFirstTime && passphraseConfirm !== passphrase) {
      setVaultError("Passphrases do not match.");
      return;
    }
    const est = estimateEntropy(passphrase);
    if (est < 40) {
      setVaultError(`Passphrase too weak: ~${Math.round(est)} bits. Use a longer passphrase (aim for 60+).`);
      return;
    }
    setLoading(true);
    try {
      await onUnlock(passphrase);
    } catch (e: any) {
      setVaultError(String(e));
    } finally {
      setLoading(false);
    }
  };

  // ─── Animated vault icon ───
  return (
    <div className="app-container">
      <div className="centered-view">
        {/* Animated vault icon */}
        <div
          className="setup-icon vault-icon"
          style={{
            animation: loading
              ? "unlockBounce 0.6s ease-in-out"
              : "pulseRing 2s ease-in-out infinite",
          }}
        >
          🔐
        </div>

        <h2>{isFirstTime ? "Set Up Your Vault" : "Unlock Your Vault"}</h2>

        <p style={{ maxWidth: 420, textAlign: "center", lineHeight: 1.6, marginBottom: 20 }}>
          {isFirstTime
            ? "Choose a strong passphrase to encrypt your identity keys and message history. This is the only key to your data."
            : "Enter your passphrase to decrypt your local data."}
          <br />
          <span style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)" }}>
            Minimum 12 characters • Argon2id key derivation
          </span>
        </p>

        <div className="vault-form">
          {/* Passphrase input */}
          <div style={{ position: "relative", width: "100%" }}>
            <Input
              id="vault-passphrase"
              type={showPassphrase ? "text" : "password"}
              placeholder="Passphrase"
              value={passphrase}
              onChange={(e) => {
                setPassphrase(e.target.value);
                setVaultError("");
              }}
              onKeyDown={(e) => e.key === "Enter" && handleUnlock()}
              autoFocus
              error={vaultError || undefined}
              clearable
              onClear={() => { setPassphrase(""); setVaultError(""); }}
            />
            <button
              onClick={() => setShowPassphrase(!showPassphrase)}
              aria-label={showPassphrase ? "Hide passphrase" : "Show passphrase"}
              style={{
                position: "absolute",
                right: 40,
                top: "50%",
                transform: "translateY(-50%)",
                background: "none",
                border: "none",
                color: "var(--color-text-muted)",
                cursor: "pointer",
                padding: "4px 8px",
                fontSize: "1.1rem",
                fontFamily: "inherit",
                zIndex: 2,
              }}
            >
              {showPassphrase ? "🙈" : "👁️"}
            </button>
          </div>

          {/* Strength meter */}
          {passphrase.length > 0 && (
            <div className="passphrase-strength" style={{ width: "100%" }}>
              <div className="strength-bar">
                <div
                  className={`strength-fill ${passphraseStrength.class}`}
                  style={{ width: `${passphraseStrength.percent}%` }}
                />
              </div>
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  width: "100%",
                }}
              >
                <span className={`strength-label ${passphraseStrength.class}`}>
                  {passphraseStrength.label}
                  {passphraseStrength.label && ` — ${passphraseStrength.bits} bits`}
                </span>
                <span style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)" }}>
                  {passphrase.length} chars
                </span>
              </div>
            </div>
          )}

          {/* Confirmation (first time only) */}
          {isFirstTime && (
            <Input
              id="vault-passphrase-confirm"
              type={showPassphrase ? "text" : "password"}
              placeholder="Confirm passphrase"
              value={passphraseConfirm}
              onChange={(e) => setPassphraseConfirm(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleUnlock()}
              error={
                passphraseConfirm && passphrase !== passphraseConfirm
                  ? "Passphrases do not match"
                  : undefined
              }
            />
          )}

          {/* Tips toggle */}
          <button
            onClick={() => setShowTips(!showTips)}
            style={{
              background: "none",
              border: "none",
              color: "var(--color-text-muted)",
              fontSize: "var(--text-sm)",
              cursor: "pointer",
              textDecoration: "underline",
              textUnderlineOffset: 2,
              fontFamily: "inherit",
              padding: 0,
            }}
          >
            {showTips ? "Hide tips" : "What makes a strong passphrase?"}
          </button>

          {showTips && (
            <div
              style={{
                background: "var(--color-bg-card)",
                padding: 12,
                borderRadius: "var(--radius-sm)",
                fontSize: "var(--text-sm)",
                color: "var(--color-text-secondary)",
                lineHeight: 1.6,
                textAlign: "left",
              }}
            >
              <strong>Tips:</strong>
              <ul style={{ margin: "4px 0 0 16px", padding: 0 }}>
                <li>Use 5+ random words (diceware method)</li>
                <li>Aim for 60+ bits of entropy</li>
                <li>Avoid common phrases or song lyrics</li>
                <li>Include a mix of cases, numbers, or symbols</li>
                <li>Passphrases like "correct-horse-battery-staple" are excellent</li>
              </ul>
            </div>
          )}

          {/* Submit button */}
          <Button
            id="vault-unlock-btn"
            onClick={handleUnlock}
            loading={loading}
            fullWidth
            style={{ marginTop: 8 }}
          >
            {isFirstTime ? "Create Vault" : "Unlock"}
          </Button>
        </div>
      </div>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
