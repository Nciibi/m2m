import { useState, useEffect } from "react";
import { Button, Input, ToastContainer } from "../components/ui";
import { LockIcon, UnlockIcon, EyeIcon, EyeOffIcon } from "../components/ui/Icons";
import { estimateEntropy } from "../utils";
import { useApp } from "../context/AppContext";
import { useVault } from "../context/VaultContext";

export default function VaultView() {
  const { identity, vaultInitialized, toasts, removeToast } = useApp();
  const { handleUnlockVault } = useVault();
  const [passphrase, setPassphrase] = useState("");
  const [passphraseConfirm, setPassphraseConfirm] = useState("");
  const [vaultError, setVaultError] = useState("");
  const [showPassphrase, setShowPassphrase] = useState(false);
  const [loading, setLoading] = useState(false);
  const [showTips, setShowTips] = useState(false);
  const [shakeKey, setShakeKey] = useState(0);
  const [strength, setStrength] = useState({ percent: 0, bits: 0, label: "", cls: "" });

  const isFirstTime = !vaultInitialized;

  useEffect(() => {
    const entropy = estimateEntropy(passphrase);
    let percent: number, label: string, cls: string;
    if (passphrase.length === 0) { percent = 0; label = ""; cls = ""; }
    else if (passphrase.length < 12) { percent = Math.min(30, passphrase.length * 5); label = "Too short (min 12)"; cls = "weak"; }
    else if (entropy < 40) { percent = 40; label = "Weak"; cls = "weak"; }
    else if (entropy < 60) { percent = 65; label = "Fair"; cls = "fair"; }
    else if (entropy < 80) { percent = 85; label = "Strong"; cls = "strong"; }
    else { percent = 100; label = "Very Strong"; cls = "very-strong"; }
    setStrength({ percent, bits: Math.round(entropy), label, cls });
  }, [passphrase]);

  const handleUnlock = async () => {
    setVaultError("");
    if (passphrase.length < 12) { setVaultError("Passphrase must be at least 12 characters."); setShakeKey(k => k + 1); return; }
    if (isFirstTime && passphraseConfirm !== passphrase) { setVaultError("Passphrases do not match."); setShakeKey(k => k + 1); return; }
    const est = estimateEntropy(passphrase);
    if (est < 40) { setVaultError(`Passphrase too weak: ~${Math.round(est)} bits. Use longer (aim for 60+).`); setShakeKey(k => k + 1); return; }
    setLoading(true);
    try { await handleUnlockVault(passphrase); }
    catch (e: any) { setVaultError(String(e)); setShakeKey(k => k + 1); }
    finally { setLoading(false); }
  };

  const colorMap: Record<string, string> = { weak: "var(--color-danger)", fair: "var(--color-warning)", strong: "var(--color-success)", "very-strong": "#22d3ee" };

  return (
    <div className="app-shell">
      <div className="centered-view">
        <div className={`vault-icon ${loading ? "vault-icon--loading" : "vault-icon--idle"}`}>
          {loading ? <UnlockIcon size={36} color="var(--color-accent-bright)" /> : <LockIcon size={36} color="var(--color-accent-bright)" />}
        </div>

        <h2 className="centered-view__title centered-view__title--spaced">
          {isFirstTime ? "Set Up Your Vault" : "Unlock Your Vault"}
        </h2>

        <p className="centered-view__desc centered-view__desc--spaced">
          {isFirstTime
            ? "Choose a strong passphrase to encrypt your identity keys and message history."
            : "Enter your passphrase to decrypt your local data."}
          <br />
          <span className="text-muted text-sm">
            Minimum 12 chars · Argon2id
          </span>
        </p>

        {/* Show fingerprint hint for returning users */}
        {!isFirstTime && identity?.fingerprint && (
          <div className="fp-hint">
            This vault belongs to {identity.fingerprint.substring(0, 16)}…
          </div>
        )}

        <div className={`vault-form ${vaultError ? "vault-form--shake" : ""}`} key={shakeKey}>
          <div className="input-wrap-relative">
            <Input
              id="vault-passphrase"
              type={showPassphrase ? "text" : "password"}
              placeholder="Passphrase"
              value={passphrase}
              onChange={e => { setPassphrase(e.target.value); setVaultError(""); }}
              onKeyDown={e => e.key === "Enter" && handleUnlock()}
              autoFocus
              error={vaultError || undefined}
              clearable
              onClear={() => { setPassphrase(""); setVaultError(""); }}
            />
            <button
              onClick={() => setShowPassphrase(!showPassphrase)}
              aria-label={showPassphrase ? "Hide" : "Show"}
              className="input__clear input__clear--absolute"
            >
              {showPassphrase ? <EyeOffIcon size={18} /> : <EyeIcon size={18} />}
            </button>
            <button
              onClick={async () => {
                try {
                  const text = await navigator.clipboard.readText();
                  setPassphrase(text);
                  setVaultError("");
                } catch { /* Clipboard access denied */ }
              }}
              className="paste-btn"
              title="Paste from clipboard"
              aria-label="Paste passphrase"
            >
              📋 Paste
            </button>
          </div>

          {passphrase.length > 0 && (
            <div className="strength-container">
              <div className="strength-bar">
                <div className="strength-fill" style={{ width: `${strength.percent}%`, background: colorMap[strength.cls] || "transparent" }} />
              </div>
              <div className="strength-info">
                <span className="strength-label" style={{ color: colorMap[strength.cls] || "var(--color-text-muted)" }}>
                  {strength.label && `${strength.label} — ${strength.bits} bits`}
                </span>
                <span className="strength-chars">{passphrase.length} chars</span>
              </div>
            </div>
          )}

          {isFirstTime && (
            <>
              <Input
                id="vault-passphrase-confirm"
                type={showPassphrase ? "text" : "password"}
                placeholder="Confirm passphrase"
                value={passphraseConfirm}
                onChange={e => setPassphraseConfirm(e.target.value)}
                onKeyDown={e => e.key === "Enter" && handleUnlock()}
                error={passphraseConfirm && passphrase !== passphraseConfirm ? "Passphrases do not match" : undefined}
              />
              {passphraseConfirm && passphrase === passphraseConfirm && passphrase.length >= 12 && (
                <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--space-xs)', fontSize: 'var(--text-sm)', color: 'var(--color-success)', marginTop: '-8px' }}>
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <polyline points="20 6 9 17 4 12"></polyline>
                  </svg>
                  Passphrases match
                </div>
              )}
            </>
          )}

          <button
            onClick={() => setShowTips(!showTips)}
            className="input__clear tips-toggle"
          >
            {showTips ? "Hide tips" : "What makes a strong passphrase?"}
          </button>

          {showTips && (
            <div className="tips-box">
              <strong>Tips:</strong>
              <ul>
                <li>Use 5+ random words (diceware method)</li>
                <li>Aim for 60+ bits of entropy</li>
                <li>Avoid common phrases or song lyrics</li>
                <li>Include a mix of cases, numbers, or symbols</li>
                <li>"correct-horse-battery-staple" style is excellent</li>
              </ul>
            </div>
          )}

          {vaultError && (
            <div className="vault-error">{vaultError}</div>
          )}

          <div className="vault-submit-wrap">
            <Button id="vault-unlock-btn" onClick={handleUnlock} loading={loading} fullWidth>
              {isFirstTime ? "Create Vault" : "Unlock"}
            </Button>
          </div>
        </div>
      </div>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
