import { useState, useEffect, useRef } from "react";
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
 * Vault unlock / first-time setup screen — S+ Edition.
 * Premium passphrase entry with animated strength meter,
 * eye toggle, copy-paste support, and error shake animation.
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
  const [isUnlocking, setIsUnlocking] = useState(false);
  const [shakeKey, setShakeKey] = useState(0);
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
      setShakeKey((k) => k + 1);
      return;
    }
    if (isFirstTime && passphraseConfirm !== passphrase) {
      setVaultError("Passphrases do not match.");
      setShakeKey((k) => k + 1);
      return;
    }
    const est = estimateEntropy(passphrase);
    if (est < 40) {
      setVaultError(
        `Passphrase too weak: ~${Math.round(est)} bits. Use a longer passphrase (aim for 60+).`
      );
      setShakeKey((k) => k + 1);
      return;
    }
    setLoading(true);
    setIsUnlocking(true);
    try {
      await onUnlock(passphrase);
    } catch (e: any) {
      setVaultError(String(e));
      setShakeKey((k) => k + 1);
      setIsUnlocking(false);
    } finally {
      setLoading(false);
    }
  };

  const strengthColors: Record<string, string> = {
    weak: "var(--color-danger)",
    fair: "var(--color-warning)",
    strong: "var(--color-success)",
    "very-strong": "#22d3ee",
  };

  return (
    <div className="app-container">
      <div className="centered-view">
        {/* Animated vault icon */}
        <div
          className="setup-icon"
          style={{
            width: 80,
            height: 80,
            borderRadius: "var(--radius-xl)",
            fontSize: "2.2rem",
            background: isUnlocking
              ? "linear-gradient(135deg, rgba(99,102,241,0.3), rgba(251,191,36,0.2))"
              : "linear-gradient(135deg, rgba(251,191,36,0.2), rgba(99,102,241,0.15))",
            border: "1px solid rgba(251,191,36,0.2)",
            boxShadow: isUnlocking
              ? "0 0 40px rgba(99,102,241,0.2)"
              : "0 0 30px rgba(251,191,36,0.1)",
            animation: loading
              ? "unlockBounce 0.6s var(--ease-out-back)"
              : "pulseRing 3s var(--ease-in-out) infinite",
            transition: "background 0.5s",
          }}
        >
          {loading ? "🔓" : "🔐"}
        </div>

        <h2 style={{ marginTop: "var(--space-lg)" }}>
          {isFirstTime ? "Set Up Your Vault" : "Unlock Your Vault"}
        </h2>

        <p
          style={{
            maxWidth: 420,
            textAlign: "center",
            lineHeight: 1.6,
            marginBottom: "var(--space-xl)",
            color: "var(--color-text-secondary)",
          }}
        >
          {isFirstTime
            ? "Choose a strong passphrase to encrypt your identity keys and message history. This is the only key to your data."
            : "Enter your passphrase to decrypt your local data."}
          <br />
          <span
            style={{
              fontSize: "var(--text-sm)",
              color: "var(--color-text-muted)",
            }}
          >
            Minimum 12 characters · Argon2id key derivation
          </span>
        </p>

        <div
          className="vault-form"
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "var(--space-sm)",
            width: "100%",
            maxWidth: 380,
          }}
          key={shakeKey}
          onAnimationEnd={() => {}}
        >
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
              onClear={() => {
                setPassphrase("");
                setVaultError("");
              }}
            />
            <button
              onClick={() => setShowPassphrase(!showPassphrase)}
              aria-label={showPassphrase ? "Hide passphrase" : "Show passphrase"}
              style={{
                position: "absolute",
                right: 44,
                top: "50%",
                transform: "translateY(-50%)",
                background: "none",
                border: "none",
                color: "var(--color-text-muted)",
                cursor: "pointer",
                padding: "4px 6px",
                fontSize: "1rem",
                fontFamily: "inherit",
                zIndex: 2,
                transition: "color var(--transition-fast)",
              }}
              onMouseEnter={(e) => { e.currentTarget.style.color = "var(--color-text-secondary)"; }}
              onMouseLeave={(e) => { e.currentTarget.style.color = "var(--color-text-muted)"; }}
            >
              {showPassphrase ? "🙈" : "👁️"}
            </button>
          </div>

          {/* Strength meter */}
          {passphrase.length > 0 && (
            <div
              style={{
                width: "100%",
                display: "flex",
                flexDirection: "column",
                gap: "var(--space-xxs)",
              }}
            >
              <div
                style={{
                  width: "100%",
                  height: 4,
                  background: "var(--color-bg-input)",
                  borderRadius: 2,
                  overflow: "hidden",
                }}
              >
                <div
                  style={{
                    height: "100%",
                    borderRadius: 2,
                    width: `${passphraseStrength.percent}%`,
                    background: strengthColors[passphraseStrength.class] || "transparent",
                    transition: "width 300ms var(--ease-out-expo), background 300ms",
                  }}
                />
              </div>
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  width: "100%",
                }}
              >
                <span
                  style={{
                    fontSize: "var(--text-xs)",
                    color: strengthColors[passphraseStrength.class] || "var(--color-text-muted)",
                    fontWeight: 500,
                  }}
                >
                  {passphraseStrength.label &&
                    `${passphraseStrength.label} — ${passphraseStrength.bits} bits`}
                </span>
                <span
                  style={{
                    fontSize: "var(--text-xs)",
                    color: "var(--color-text-muted)",
                  }}
                >
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
              alignSelf: "flex-start",
              transition: "color var(--transition-fast)",
            }}
            onMouseEnter={(e) => { e.currentTarget.style.color = "var(--color-text-secondary)"; }}
            onMouseLeave={(e) => { e.currentTarget.style.color = "var(--color-text-muted)"; }}
          >
            {showTips ? "Hide tips" : "What makes a strong passphrase? ▼"}
          </button>

          {showTips && (
            <div
              style={{
                background: "var(--color-bg-card)",
                padding: "var(--space-md)",
                borderRadius: "var(--radius-md)",
                fontSize: "var(--text-sm)",
                color: "var(--color-text-secondary)",
                lineHeight: 1.6,
                textAlign: "left",
                border: "1px solid var(--color-border-default)",
                animation: "expandDown 300ms var(--ease-out-expo)",
                overflow: "hidden",
              }}
            >
              <strong style={{ color: "var(--color-text-primary)" }}>Tips:</strong>
              <ul style={{ margin: "var(--space-xxs) 0 0 var(--space-lg)", padding: 0 }}>
                <li>Use 5+ random words (diceware method)</li>
                <li>Aim for 60+ bits of entropy</li>
                <li>Avoid common phrases or song lyrics</li>
                <li>Include a mix of cases, numbers, or symbols</li>
                <li>
                  Passphrases like{" "}
                  <code
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: "var(--text-xs)",
                      background: "var(--color-bg-input)",
                      padding: "1px 6px",
                      borderRadius: 3,
                    }}
                  >
                    correct-horse-battery-staple
                  </code>{" "}
                  are excellent
                </li>
              </ul>
            </div>
          )}

          {/* Error display */}
          {vaultError && shakeKey > 0 && (
            <div
              style={{
                color: "var(--color-danger)",
                fontSize: "var(--text-sm)",
                padding: "var(--space-xs) var(--space-sm)",
                background: "var(--color-danger-bg)",
                border: "1px solid rgba(239,68,68,0.2)",
                borderRadius: "var(--radius-sm)",
                animation: "shake 0.4s var(--ease-out-expo)",
              }}
            >
              {vaultError}
            </div>
          )}

          {/* Submit button */}
          <Button
            id="vault-unlock-btn"
            onClick={handleUnlock}
            loading={loading}
            fullWidth
            style={{ marginTop: "var(--space-xs)" }}
          >
            {isFirstTime ? "Create Vault" : "Unlock"}
          </Button>
        </div>
      </div>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
