import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ToastContainer } from "../components/ui";
import { estimateEntropy } from "../utils";
import { useApp } from "../context/AppContext";
import { useVault } from "../context/VaultContext";

export default function VaultView() {
  const { vaultInitialized, toasts, removeToast, addToast, setView } = useApp();
  const { handleUnlockVault } = useVault();

  const [passphrase, setPassphrase] = useState("");
  const [passphraseConfirm, setPassphraseConfirm] = useState("");
  const [loading, setLoading] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState("");
  const [shake, setShake] = useState(false);

  const entropy = estimateEntropy(passphrase);
  let entropyColor = "var(--color-danger)";
  let entropyLabel = "Weak";
  if (entropy > 40) { entropyColor = "var(--color-warning)"; entropyLabel = "Fair"; }
  if (entropy > 60) { entropyColor = "var(--color-success)"; entropyLabel = "Strong"; }

  const triggerShake = () => {
    setShake(true);
    setTimeout(() => setShake(false), 500);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (loading) return;
    setError("");

    if (!vaultInitialized) {
      if (passphrase.length < 12) {
        setError("Passphrase must be at least 12 characters.");
        triggerShake();
        return;
      }
      if (passphrase !== passphraseConfirm) {
        setError("Passphrases do not match.");
        triggerShake();
        return;
      }
    }

    setLoading(true);
    try {
      await handleUnlockVault(passphrase);
      setPassphrase("");
      setPassphraseConfirm("");
    } catch (err: any) {
      const msg = typeof err === "string" ? err : err?.message || "Unlock failed. Check your passphrase.";
      setError(msg);
      triggerShake();
      addToast(msg, "error");
    } finally {
      setLoading(false);
    }
  };

  return (
    <main className="relative z-10 w-full flex justify-center items-center h-full min-h-screen px-gutter">
      {/* Unlock Vault Card */}
      <div className={`premium-glass-card rounded-3xl max-w-[420px] w-full p-2xl flex flex-col items-center relative group ${shake ? "animate-[shake_0.4s_ease]" : ""}`}>

        {/* Icon */}
        <div className="w-20 h-20 rounded-full flex items-center justify-center bg-input-bg border border-border-subtle mb-xl">
          <span className="material-symbols-outlined text-primary text-4xl">{vaultInitialized ? "lock" : "key"}</span>
        </div>

        {/* Header */}
        <div className="text-center mb-xl space-y-sm">
          <h1 className="font-headline-3xl text-headline-3xl text-on-surface tracking-tight font-bold">
            {vaultInitialized ? "Unlock Your Vault" : "Setup Your Vault"}
          </h1>
          <p className="font-body-md text-body-md text-on-surface-variant">
            {vaultInitialized
              ? "Enter your passphrase to decrypt your local data."
              : "Create a strong passphrase to encrypt your identity."}
          </p>
          <div className="pt-sm">
            <span className="font-mono-label text-label-xs bg-input-bg px-2 py-1 rounded-md text-text-muted border border-border-subtle">
              {vaultInitialized ? "Argon2id · AES-256-GCM" : "Minimum 12 chars · Argon2id"}
            </span>
          </div>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="w-full space-y-lg">
          {/* Passphrase Input */}
          <div className="space-y-xs">
            <label htmlFor="passphrase" className="font-label-sm text-label-sm text-on-surface-variant uppercase tracking-wider">
              Passphrase
            </label>
            <div className="relative">
              <input
                className="w-full bg-input-bg border border-border-subtle rounded-xl px-lg py-md pr-12 font-mono-code text-body-lg text-primary placeholder:text-text-muted/30 focus:outline-none focus:ring-2 focus:ring-primary transition-all duration-300"
                id="passphrase"
                placeholder="••••••••••••"
                required
                autoComplete="current-password"
                type={showPassword ? "text" : "password"}
                value={passphrase}
                onChange={e => { setPassphrase(e.target.value); setError(""); }}
                minLength={vaultInitialized ? 1 : 12}
              />
              <button
                className="absolute right-3 top-1/2 -translate-y-1/2 text-text-muted hover:text-on-surface transition-colors duration-200 p-1"
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                tabIndex={-1}
              >
                <span className="material-symbols-outlined text-[20px]">{showPassword ? "visibility_off" : "visibility"}</span>
              </button>
            </div>
          </div>

          {/* Confirm Passphrase (setup only) */}
          {!vaultInitialized && (
            <div className="space-y-xs">
              <label className="font-label-sm text-label-sm text-on-surface-variant uppercase tracking-wider">
                Confirm Passphrase
              </label>
              <input
                className="w-full bg-input-bg border border-border-subtle rounded-xl px-lg py-md font-mono-code text-body-lg text-primary placeholder:text-text-muted/30 focus:outline-none focus:ring-2 focus:ring-primary transition-all duration-300"
                placeholder="Re-enter passphrase"
                required
                autoComplete="new-password"
                type={showPassword ? "text" : "password"}
                value={passphraseConfirm}
                onChange={e => { setPassphraseConfirm(e.target.value); setError(""); }}
                minLength={12}
              />
              {/* Entropy bar */}
              {passphrase.length > 0 && (
                <div className="space-y-xs pt-xs">
                  <div className="w-full h-1.5 bg-input-bg rounded-full overflow-hidden">
                    <div
                      className="h-full rounded-full transition-all duration-500"
                      style={{ width: `${Math.min(100, (entropy / 80) * 100)}%`, background: entropyColor }}
                    />
                  </div>
                  <div className="flex justify-between text-[11px]">
                    <span style={{ color: entropyColor }}>{entropyLabel} · {entropy.toFixed(0)} bits</span>
                    {passphraseConfirm.length > 0 && passphrase !== passphraseConfirm && (
                      <span className="text-danger font-bold">Does not match</span>
                    )}
                    {passphrase.length > 0 && passphraseConfirm.length > 0 && passphrase === passphraseConfirm && (
                      <span className="text-tertiary font-bold flex items-center gap-1">
                        <span className="material-symbols-outlined text-[12px]">check_circle</span> Match
                      </span>
                    )}
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Error Message */}
          {error && (
            <div className="flex items-center gap-sm p-md bg-error-container border border-error rounded-xl text-danger text-body-md animate-in fade-in slide-in-from-top-2 duration-200">
              <span className="material-symbols-outlined text-[18px]">error</span>
              <span>{error}</span>
            </div>
          )}

          {/* Submit Button */}
          <button
            disabled={loading || (!vaultInitialized && (passphrase !== passphraseConfirm || passphrase.length < 12))}
            className="premium-btn w-full h-14 rounded-xl text-white bg-gradient-to-r from-primary-container to-inverse-primary font-headline-2xl font-bold flex items-center justify-center gap-md hover:brightness-125 transition-all duration-300 shadow-[0_0_20px_rgba(99,102,241,0.2)] hover:shadow-[0_0_30px_rgba(99,102,241,0.5)] disabled:opacity-50 disabled:cursor-not-allowed"
            type="submit"
          >
            {loading ? (
              <>
                <span className="material-symbols-outlined animate-spin text-[20px] relative z-10">sync</span>
                <span className="relative z-10">{vaultInitialized ? "Decrypting..." : "Generating Keys..."}</span>
              </>
            ) : (
              <span className="relative z-10">{vaultInitialized ? "Unlock Enclave" : "Initialize Enclave"}</span>
            )}
          </button>
        </form>

        <div className="mt-xl text-center">
          <p className="font-mono-label text-[10px] text-primary/30 uppercase tracking-[0.2em]">AES-256-GCM / Argon2id</p>
        </div>
      </div>

      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </main>
  );
}
