import { useState, useEffect } from "react";
import { ToastContainer } from "../components/ui";
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
  const [shakeKey, setShakeKey] = useState(0);

  const isFirstTime = !vaultInitialized;

  const handleUnlock = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    setVaultError("");
    if (passphrase.length < 12) { setVaultError("Passphrase must be at least 12 characters."); setShakeKey(k => k + 1); return; }
    if (isFirstTime && passphraseConfirm !== passphrase) { setVaultError("Passphrases do not match."); setShakeKey(k => k + 1); return; }
    const est = estimateEntropy(passphrase);
    if (est < 40) { setVaultError(`Passphrase too weak: ~${Math.round(est)} bits. Use longer (aim for 60+).`); setShakeKey(k => k + 1); return; }
    setLoading(true);
    try { await handleUnlockVault(passphrase); }
    catch (err: any) { setVaultError(String(err)); setShakeKey(k => k + 1); }
    finally { setLoading(false); }
  };

  return (
    <div style={{ display: 'flex', width: '100%', minHeight: '100vh', alignItems: 'center', justifyContent: 'center', position: 'relative', overflow: 'hidden', background: 'var(--color-bg-dark)' }}>
      {/* Atmospheric Background Elements */}
      <div style={{ position: 'absolute', top: '-10%', left: '-10%', width: '40%', height: '40%', background: 'radial-gradient(circle at center, rgba(99, 102, 241, 0.15) 0%, transparent 70%)', pointerEvents: 'none' }} />
      <div style={{ position: 'absolute', bottom: '-10%', right: '-10%', width: '40%', height: '40%', background: 'radial-gradient(circle at center, rgba(99, 102, 241, 0.15) 0%, transparent 70%)', pointerEvents: 'none' }} />
      
      <main style={{ position: 'relative', zIndex: 10, padding: '0 16px', width: '100%', display: 'flex', justifyContent: 'center' }}>
        {/* Unlock Vault Card */}
        <div style={{ 
          background: 'rgba(12, 14, 24, 0.82)', 
          backdropFilter: 'blur(24px)', 
          border: '1px solid rgba(255, 255, 255, 0.08)', 
          maxWidth: '380px', 
          width: '100%', 
          borderRadius: '24px', 
          padding: '32px', 
          boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.5)', 
          display: 'flex', 
          flexDirection: 'column', 
          alignItems: 'center',
          animation: shakeKey ? `shake 0.5s ease-in-out` : undefined
        }} key={shakeKey}>
          
          {/* Icon Container */}
          <div style={{ 
            width: '80px', 
            height: '80px', 
            borderRadius: '50%', 
            display: 'flex', 
            alignItems: 'center', 
            justifyContent: 'center', 
            background: 'rgba(255, 255, 255, 0.05)', 
            border: '1px solid rgba(255, 255, 255, 0.1)', 
            marginBottom: '24px',
            animation: 'pulseRing 4s ease-in-out infinite'
          }}>
            <span className="material-symbols-outlined" style={{ color: 'var(--color-primary)', fontSize: '40px' }}>
              {loading ? 'lock_open' : 'lock'}
            </span>
          </div>

          {/* Typography Header */}
          <div style={{ textAlign: 'center', marginBottom: '24px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
            <h1 style={{ fontSize: '24px', fontWeight: 700, color: 'var(--color-text-primary)', letterSpacing: '-0.02em', margin: 0 }}>
              {isFirstTime ? "Set Up Your Vault" : "Unlock Your Vault"}
            </h1>
            <p style={{ fontSize: '14px', color: 'rgba(99, 102, 241, 0.7)', padding: '0 16px', margin: 0, lineHeight: 1.5 }}>
              {isFirstTime ? "Choose a strong passphrase to encrypt your identity." : "Enter your passphrase to decrypt your local data."}
            </p>
            <div style={{ paddingTop: '8px' }}>
              <span style={{ fontFamily: 'var(--font-mono)', fontSize: '11px', background: 'rgba(255, 255, 255, 0.05)', padding: '4px 8px', borderRadius: '6px', color: 'var(--color-text-muted)', border: '1px solid rgba(255, 255, 255, 0.05)' }}>
                Minimum 12 chars · Argon2id
              </span>
            </div>
          </div>

          {/* Form */}
          <form style={{ width: '100%', display: 'flex', flexDirection: 'column', gap: '16px' }} onSubmit={handleUnlock}>
            <div style={{ position: 'relative' }}>
              <input 
                type={showPassphrase ? "text" : "password"}
                value={passphrase}
                onChange={e => { setPassphrase(e.target.value); setVaultError(""); }}
                placeholder="••••••••••••"
                required
                style={{ 
                  width: '100%', 
                  background: 'rgba(255, 255, 255, 0.05)', 
                  border: `1px solid ${vaultError ? 'var(--color-danger)' : 'rgba(255, 255, 255, 0.1)'}`, 
                  borderRadius: '12px', 
                  padding: '12px 16px', 
                  fontFamily: 'var(--font-mono)', 
                  fontSize: '16px', 
                  color: 'var(--color-primary)', 
                  outline: 'none',
                  transition: 'all 0.3s'
                }}
              />
              <button 
                type="button" 
                onClick={() => setShowPassphrase(!showPassphrase)}
                style={{ position: 'absolute', right: '12px', top: '50%', transform: 'translateY(-50%)', background: 'none', border: 'none', color: 'var(--color-text-muted)', cursor: 'pointer' }}
              >
                <span className="material-symbols-outlined" style={{ fontSize: '20px' }}>{showPassphrase ? 'visibility_off' : 'visibility'}</span>
              </button>
            </div>

            {isFirstTime && (
              <div style={{ position: 'relative' }}>
                <input 
                  type={showPassphrase ? "text" : "password"}
                  value={passphraseConfirm}
                  onChange={e => { setPassphraseConfirm(e.target.value); setVaultError(""); }}
                  placeholder="Confirm passphrase"
                  required
                  style={{ 
                    width: '100%', 
                    background: 'rgba(255, 255, 255, 0.05)', 
                    border: `1px solid ${vaultError ? 'var(--color-danger)' : 'rgba(255, 255, 255, 0.1)'}`, 
                    borderRadius: '12px', 
                    padding: '12px 16px', 
                    fontFamily: 'var(--font-mono)', 
                    fontSize: '16px', 
                    color: 'var(--color-primary)', 
                    outline: 'none',
                    transition: 'all 0.3s'
                  }}
                />
              </div>
            )}

            {vaultError && (
              <div style={{ color: 'var(--color-danger)', fontSize: '13px', textAlign: 'center' }}>
                {vaultError}
              </div>
            )}

            <button 
              type="submit" 
              disabled={loading}
              style={{ 
                width: '100%', 
                height: '56px', 
                borderRadius: '12px', 
                background: 'linear-gradient(to top right, #4f46e5, #6366f1)', 
                color: 'white', 
                fontSize: '18px', 
                fontWeight: 700, 
                display: 'flex', 
                alignItems: 'center', 
                justifyContent: 'center', 
                gap: '12px', 
                border: 'none', 
                cursor: loading ? 'not-allowed' : 'pointer',
                boxShadow: '0 10px 25px -5px rgba(99, 102, 241, 0.4)',
                transition: 'all 0.2s',
                opacity: loading ? 0.8 : 1
              }}
            >
              {loading ? (
                <>
                  <span className="material-symbols-outlined" style={{ animation: 'spin 1s linear infinite' }}>sync</span>
                  Decrypting...
                </>
              ) : isFirstTime ? "Create Vault" : "Unlock"}
            </button>
          </form>

          {/* Card Footer */}
          {!isFirstTime && identity?.fingerprint && (
            <div style={{ marginTop: '32px', textAlign: 'center' }}>
              <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '4px' }}>
                <span style={{ fontFamily: 'var(--font-mono)', fontSize: '10px', color: 'rgba(148, 163, 184, 0.4)', letterSpacing: '0.1em', textTransform: 'uppercase' }}>Vault Fingerprint</span>
                <p style={{ fontFamily: 'var(--font-mono)', fontSize: '11px', color: 'rgba(148, 163, 184, 0.3)', margin: 0 }}>
                  {identity.fingerprint.substring(0, 32)}...
                </p>
              </div>
            </div>
          )}
        </div>
      </main>

      {/* Decorative Tag */}
      <div style={{ position: 'fixed', bottom: '24px', left: '24px', display: 'none' }} className="md-block-decorative">
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', color: 'rgba(148, 163, 184, 0.2)', fontFamily: 'var(--font-mono)', fontSize: '11px' }}>
          <span className="material-symbols-outlined" style={{ fontSize: '14px' }}>verified_user</span>
          <span>E2EE PROTOCOL ACTIVE</span>
        </div>
      </div>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
