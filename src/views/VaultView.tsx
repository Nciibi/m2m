import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ToastContainer } from "../components/ui";
import { estimateEntropy } from "../utils";
import { useApp } from "../context/AppContext";
import { useVault } from "../context/VaultContext";

export default function VaultView() {
  const { vaultInitialized, toasts, removeToast, setView } = useApp();
  const { handleUnlockVault } = useVault();

  const [passphrase, setPassphrase] = useState("");
  const [passphraseConfirm, setPassphraseConfirm] = useState("");
  const [loading, setLoading] = useState(false);
  const [showPassword, setShowPassword] = useState(false);

  const entropy = estimateEntropy(passphrase);
  let entropyColor = "var(--color-danger)";
  if (entropy > 40) entropyColor = "var(--color-warning)";
  if (entropy > 60) entropyColor = "var(--color-success)";

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (loading) return;

    if (!vaultInitialized) {
      if (passphrase !== passphraseConfirm) return;
      if (passphrase.length < 12) return;
    }

    setLoading(true);
    try {
      if (vaultInitialized) {
        await handleUnlockVault(passphrase);
      } else {
        await invoke("setup_vault", { passphrase });
        setView("hub");
      }
      setPassphrase("");
      setPassphraseConfirm("");
    } catch (error) {
    } finally {
      setLoading(false);
    }
  };

  return (
    <main className="relative z-10 px-gutter w-full flex justify-center items-center min-h-screen">      
      {/* Unlock Vault Card */}
      <div className="max-w-[380px] w-full rounded-3xl p-xl shadow-[0_0_50px_-12px_rgba(0,0,0,0.8)] border border-white/5 bg-surface/60 backdrop-blur-[60px] saturate-[1.2] flex flex-col items-center animate-in fade-in zoom-in-95 duration-500 relative overflow-hidden group">
        <div className="absolute top-0 inset-x-0 h-[1px] bg-gradient-to-r from-transparent via-primary/50 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-1000"></div>
        {/* Icon Container */}
        <div className="w-20 h-20 rounded-full flex items-center justify-center bg-white/5 border border-white/10 mb-xl animate-pulse">
          <span className="material-symbols-outlined text-primary text-4xl">{vaultInitialized ? "lock" : "key"}</span>
        </div>
        
        {/* Typography Header */}
        <div className="text-center mb-xl space-y-sm">
          <h1 className="font-headline-2xl text-headline-2xl text-text-primary tracking-tight">
            {vaultInitialized ? "Unlock Your Vault" : "Setup Your Vault"}
          </h1>
          <p className="font-body-md text-body-md px-4 text-primary/70">
            {vaultInitialized ? "Enter your passphrase to decrypt your local data." : "Create a strong passphrase to encrypt your identity."}
          </p>
          <div className="pt-sm">
            <span className="font-mono-label text-label-xs bg-white/5 px-2 py-1 rounded-md text-text-muted border border-white/5">
              Minimum 12 chars · Argon2id
            </span>
          </div>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="w-full space-y-lg">
          <div className="relative">
            <input 
              className="w-full bg-input-bg border border-white/10 rounded-xl px-lg py-md font-mono-code text-body-lg text-primary placeholder:text-text-muted/30 focus:outline-none focus:ring-2 focus:ring-primary/50 transition-all duration-300" 
              id="passphrase" 
              placeholder="••••••••••••" 
              required 
              type={showPassword ? "text" : "password"}
              value={passphrase}
              onChange={e => setPassphrase(e.target.value)}
              minLength={12}
            />
            <button 
              className="absolute right-md top-1/2 -translate-y-1/2 text-text-muted hover:text-text-primary transition-colors duration-200" 
              type="button"
              onClick={() => setShowPassword(!showPassword)}
            >
              <span className="material-symbols-outlined text-[20px]">{showPassword ? "visibility_off" : "visibility"}</span>
            </button>
          </div>

          {!vaultInitialized && (
            <div className="relative mt-4">
              <input 
                className="w-full bg-input-bg border border-white/10 rounded-xl px-lg py-md font-mono-code text-body-lg text-primary placeholder:text-text-muted/30 focus:outline-none focus:ring-2 focus:ring-primary/50 transition-all duration-300" 
                placeholder="Confirm Passphrase" 
                required 
                type={showPassword ? "text" : "password"}
                value={passphraseConfirm}
                onChange={e => setPassphraseConfirm(e.target.value)}
                minLength={12}
              />
              <div className="mt-2 text-xs text-center" style={{ color: entropyColor }}>
                Entropy: {entropy.toFixed(1)} bits
              </div>
            </div>
          )}

          <button 
            disabled={loading || (!vaultInitialized && (passphrase !== passphraseConfirm || passphrase.length < 12))}
            className="w-full h-14 rounded-xl text-on-primary-container bg-gradient-to-r from-primary-container to-inverse-primary font-headline-2xl font-bold flex items-center justify-center gap-md hover:brightness-125 active:scale-[0.98] transition-all duration-300 shadow-[0_0_20px_rgba(99,102,241,0.2)] hover:shadow-[0_0_30px_rgba(99,102,241,0.5)] disabled:opacity-50 disabled:cursor-not-allowed group/btn relative overflow-hidden" 
            type="submit"
          >
            <div className="absolute inset-0 bg-white opacity-0 group-hover/btn:opacity-[0.03] transition-opacity"></div>
            {loading ? (
              <>
                <span className="material-symbols-outlined animate-spin text-[20px]">sync</span>
                <span className="ml-2">{vaultInitialized ? "Decrypting..." : "Generating Keys..."}</span>
              </>
            ) : (
              vaultInitialized ? "Unlock Enclave" : "Initialize Enclave"
            )}
          </button>
        </form>

        <div className="mt-xl text-center space-y-sm">
          <p className="font-mono-label text-[10px] text-primary/30 uppercase tracking-[0.2em]">AES-256-GCM / Argon2id</p>
        </div>
      </div>
      
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </main>
  );
}
