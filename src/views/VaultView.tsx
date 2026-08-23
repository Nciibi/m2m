import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, Input, ToastContainer, OnScreenKeyboard } from "../components/ui";
import { LockIcon, UnlockIcon, EyeIcon, EyeOffIcon, CheckIcon } from "../components/ui/Icons";
import { estimateEntropy } from "../utils";
import { useApp } from "../context/AppContext";
import { useVault } from "../context/VaultContext";

export default function VaultView() {
  const { identity, vaultInitialized, setView, toasts, removeToast, addToast } = useApp();
  const { handleUnlockVault } = useVault();
  const [passphrase, setPassphrase] = useState("");
  const [passphraseConfirm, setPassphraseConfirm] = useState("");
  const [vaultError, setVaultError] = useState("");
  const [showPassphrase, setShowPassphrase] = useState(false);
  const [loading, setLoading] = useState(false);
  const [showTips, setShowTips] = useState(false);
  const [shaking, setShaking] = useState(false);
  const [strength, setStrength] = useState({ percent: 0, bits: 0, label: "", cls: "" });
  const [createMode, setCreateMode] = useState(false);
  // On-screen keyboard state + which field it targets ("main" | "confirm").
  const [oskOpen, setOskOpen] = useState(false);
  const [oskTarget, setOskTarget] = useState<"main" | "confirm">("main");

  const isFirstTime = !vaultInitialized;
  const showConfirm = isFirstTime || createMode;

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

  const fail = (msg: string) => {
    setVaultError(msg);
    // Re-trigger the shake without remounting (keeps input focus).
    setShaking(false);
    requestAnimationFrame(() => setShaking(true));
  };

  const handleUnlock = async () => {
    setVaultError("");
    if (passphrase.length < 12) { fail("Passphrase must be at least 12 characters."); return; }
    if (showConfirm && passphraseConfirm !== passphrase) { fail("Passphrases do not match."); return; }
    const est = estimateEntropy(passphrase);
    if (est < 40) { fail(`Passphrase too weak: ~${Math.round(est)} bits. Use longer (aim for 60+).`); return; }
    setLoading(true);
    try {
      if (createMode) {
        await invoke("create_vault_account", { passphrase });
        setCreateMode(false);
        setView("hub");
      } else {
        await handleUnlockVault(passphrase);
      }
      setPassphrase("");
      setPassphraseConfirm("");
    } catch (e: any) {
      const msg = typeof e === "string"
        ? e
        : e?.message || (createMode ? "Account creation failed." : "Unlock failed. Check your passphrase.");
      fail(msg);
      addToast(msg, "error");
    } finally {
      setLoading(false);
    }
  };

  const colorMap: Record<string, string> = { weak: "var(--color-danger)", fair: "var(--color-warning)", strong: "var(--color-success)", "very-strong": "#22d3ee" };
  const confirmMismatch = passphraseConfirm.length > 0 && passphraseConfirm !== passphrase;

  // On-screen keyboard insertion — routes to the focused/targeted field and
  // never lets keystrokes touch the physical keyboard event path.
  const oskInsert = (ch: string) => {
    setVaultError("");
    if (oskTarget === "confirm") setPassphraseConfirm((p) => p + ch);
    else setPassphrase((p) => p + ch);
  };
  const oskBackspace = () => {
    if (oskTarget === "confirm") setPassphraseConfirm((p) => p.slice(0, -1));
    else setPassphrase((p) => p.slice(0, -1));
  };

  return (
    <div className="app-shell">
      <div className="centered-view">
        <div className={`vault-icon ${loading ? "vault-icon--loading" : "vault-icon--idle"}`}>
          {loading ? <UnlockIcon size={36} color="var(--color-accent-bright)" /> : <LockIcon size={36} color="var(--color-accent-bright)" />}
        </div>

        <h2 className="centered-view__title centered-view__title--spaced vault-title">
          {createMode ? "Create Another Account" : isFirstTime ? "Set Up Your Vault" : "Unlock Your Vault"}
        </h2>

        <p className="centered-view__desc vault-desc">
          {createMode
            ? "Choose a strong passphrase for the new account — it selects this account on unlock."
            : isFirstTime
            ? "Choose a strong passphrase to encrypt your identity keys and message history."
            : "Enter your passphrase to decrypt your local data."}
        </p>

        <p className="vault-crypto-hint">Minimum 12 chars · Argon2id</p>

        {!isFirstTime && !createMode && identity?.fingerprint && (
          <div className="fp-hint">
            This vault belongs to {identity.fingerprint.substring(0, 16)}…
          </div>
        )}

        <div
          className={`vault-form ${shaking ? "vault-form--shake" : ""}`}
          onAnimationEnd={(e) => { if (e.animationName === "shake") setShaking(false); }}
        >
          {/* ── Passphrase field ── */}
          <div className="vault-field">
            <label className="vault-field__label" htmlFor="vault-passphrase">Passphrase</label>
            <div className="vault-input-wrap">
              <Input
                id="vault-passphrase"
                type={showPassphrase ? "text" : "password"}
                placeholder="Enter your passphrase"
                value={passphrase}
                onChange={e => { setPassphrase(e.target.value); setVaultError(""); }}
                onKeyDown={e => e.key === "Enter" && handleUnlock()}
                onFocus={() => setOskTarget("main")}
                autoFocus
              />
              <div className="vault-input-actions">
                <button
                  onClick={() => { setOskOpen(o => !o); setOskTarget("main"); }}
                  className="vault-paste-btn"
                  title="On-screen keyboard (bypasses hardware keyloggers)"
                  aria-label="Toggle on-screen keyboard"
                >
                  ⌨
                </button>
                <button
                  onClick={async () => {
                    try {
                      const text = await navigator.clipboard.readText();
                      setPassphrase(text);
                      setVaultError("");
                    } catch { /* clipboard unavailable */ }
                  }}
                  className="vault-paste-btn"
                  title="Paste from clipboard"
                  aria-label="Paste passphrase"
                >
                  Paste
                </button>
                <button
                  onClick={() => setShowPassphrase(!showPassphrase)}
                  aria-label={showPassphrase ? "Hide passphrase" : "Show passphrase"}
                  aria-pressed={showPassphrase}
                  className="vault-toggle-btn"
                >
                  {showPassphrase ? <EyeOffIcon size={18} /> : <EyeIcon size={18} />}
                </button>
              </div>
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
          </div>

          {/* ── Confirm field (first time or create mode) ── */}
          {showConfirm && (
            <div className="vault-field">
              <label className="vault-field__label" htmlFor="vault-passphrase-confirm">Confirm</label>
              <div className="vault-input-wrap">
              <Input
                id="vault-passphrase-confirm"
                type={showPassphrase ? "text" : "password"}
                placeholder="Repeat your passphrase"
                value={passphraseConfirm}
                onChange={e => setPassphraseConfirm(e.target.value)}
                onKeyDown={e => e.key === "Enter" && handleUnlock()}
                onFocus={() => setOskTarget("confirm")}
                error={confirmMismatch ? "Passphrases do not match" : undefined}
              />
              {passphraseConfirm && passphrase === passphraseConfirm && passphrase.length >= 12 && (
                <span className="vault-match-check">
                  <CheckIcon size={14} color="var(--color-success)" />
                </span>
              )}
            </div>
          </div>
        )}

        {oskOpen && (
          <OnScreenKeyboard
            open={oskOpen}
            onInsert={oskInsert}
            onBackspace={oskBackspace}
            onClose={() => setOskOpen(false)}
          />
        )}

          {vaultError && <div className="vault-error" role="alert">{vaultError}</div>}

          <button type="button" onClick={() => setShowTips(!showTips)} className="vault-tips-toggle"
            aria-expanded={showTips}>
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

          <div className="vault-submit-wrap">
            <Button id="vault-unlock-btn" onClick={handleUnlock} loading={loading} fullWidth>
              {createMode ? "Create Account" : isFirstTime ? "Create Vault" : "Unlock"}
            </Button>
          </div>

          {!isFirstTime && (
            <>
              <div className="vault-divider" role="separator" />
              <Button
                variant="ghost"
                onClick={() => {
                  setCreateMode(!createMode);
                  setVaultError("");
                  setPassphrase("");
                  setPassphraseConfirm("");
                }}
                fullWidth
              >
                {createMode ? "Back to unlock" : "Create another account"}
              </Button>
            </>
          )}
        </div>
      </div>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
