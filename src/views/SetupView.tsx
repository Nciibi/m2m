import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ToastContainer } from "../components/ui";
import { useApp } from "../context/AppContext";

const STEPS = [
  { title: "Welcome to M2M", desc: "A private, end-to-end encrypted messenger. No servers, no accounts, no tracking.", icon: "rocket_launch" },
  { title: "Your Identity is Local", desc: "Your keys are generated on this device and never leave it.", icon: "vpn_key" },
  { title: "End-to-End Encrypted", desc: "Messages use X3DH + Double Ratchet. Ed25519 signing, X25519 key exchange, XChaCha20-Poly1305 encryption.", icon: "lock" },
  { title: "Ready to Go!", desc: "Share your invite link with a trusted peer to start chatting.", icon: "check_circle" },
];

export default function SetupView() {
  const { toasts, removeToast } = useApp();
  const [loading, setLoading] = useState(true);
  const [step, setStep] = useState(0);
  const [isFirstRun, setIsFirstRun] = useState(false);
  const [slideDir, setSlideDir] = useState<"right" | "left">("right");

  useEffect(() => {
    invoke<boolean>("is_first_run")
      .then((first) => {
        setIsFirstRun(first);
        if (!first) setStep(3);
      })
      .catch(() => {})
      .finally(() => setTimeout(() => setLoading(false), 2200));
  }, []);

  const goNext = () => { setSlideDir("right"); if (step < STEPS.length - 1) setStep(s => s + 1); };
  const goBack = () => { setSlideDir("left"); if (step > 0) setStep(s => s - 1); };

  if (loading) {
    return (
      <div style={{ display: 'flex', width: '100%', minHeight: '100vh', alignItems: 'center', justifyContent: 'center', position: 'relative', overflow: 'hidden', background: 'var(--color-bg-dark)' }}>
        {/* Atmospheric Background Elements */}
        <div style={{ position: 'absolute', top: '-10%', left: '-10%', width: '40%', height: '40%', background: 'radial-gradient(circle at center, rgba(99, 102, 241, 0.15) 0%, transparent 70%)', pointerEvents: 'none' }} />
        <div style={{ position: 'absolute', bottom: '-10%', right: '-10%', width: '40%', height: '40%', background: 'radial-gradient(circle at center, rgba(99, 102, 241, 0.15) 0%, transparent 70%)', pointerEvents: 'none' }} />
        
        <main style={{ position: 'relative', zIndex: 10, padding: '0 16px', width: '100%', display: 'flex', justifyContent: 'center' }}>
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', textAlign: 'center' }}>
            <div style={{ width: '80px', height: '80px', borderRadius: '50%', display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(255, 255, 255, 0.05)', border: '1px solid rgba(255, 255, 255, 0.1)', marginBottom: '24px', animation: 'pulseRing 2s ease-in-out infinite' }}>
              <span className="material-symbols-outlined" style={{ color: 'var(--color-primary)', fontSize: '40px' }}>vpn_key</span>
            </div>
            <h2 style={{ fontSize: '24px', fontWeight: 700, color: 'var(--color-text-primary)', marginBottom: '8px' }}>Initializing Secure Enclave</h2>
            <p style={{ fontSize: '14px', color: 'var(--color-text-secondary)', marginBottom: '24px' }}>Generating Ed25519 identity keys.<br />They never leave your device.</p>
            <div style={{ display: 'flex', gap: '6px' }}>
              <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: 'var(--color-accent)', animation: 'dotBounce 1.4s ease-in-out infinite both' }} />
              <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: 'var(--color-accent)', animation: 'dotBounce 1.4s ease-in-out infinite both', animationDelay: '0.16s' }} />
              <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: 'var(--color-accent)', animation: 'dotBounce 1.4s ease-in-out infinite both', animationDelay: '0.32s' }} />
            </div>
            <div style={{ marginTop: '32px', fontFamily: 'var(--font-mono)', fontSize: '11px', background: 'rgba(255, 255, 255, 0.05)', padding: '4px 8px', borderRadius: '6px', color: 'var(--color-text-muted)', border: '1px solid rgba(255, 255, 255, 0.05)' }}>
              Ed25519 · X25519 · XChaCha20-Poly1305
            </div>
          </div>
        </main>
        <ToastContainer toasts={toasts} onRemove={removeToast} />
      </div>
    );
  }

  if (!isFirstRun) {
    return (
      <div style={{ display: 'flex', width: '100%', minHeight: '100vh', alignItems: 'center', justifyContent: 'center', position: 'relative', overflow: 'hidden', background: 'var(--color-bg-dark)' }}>
        {/* Minimal loading state if it's not first run but we need to wait briefly before routing */}
        <div style={{ width: '80px', height: '80px', borderRadius: '50%', display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(255, 255, 255, 0.05)', border: '1px solid rgba(255, 255, 255, 0.1)', animation: 'pulseRing 2s ease-in-out infinite' }}>
          <span className="material-symbols-outlined" style={{ color: 'var(--color-primary)', fontSize: '40px' }}>lock</span>
        </div>
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', width: '100%', minHeight: '100vh', alignItems: 'center', justifyContent: 'center', position: 'relative', overflow: 'hidden', background: 'var(--color-bg-dark)' }}>
      {/* Atmospheric Background Elements */}
      <div style={{ position: 'absolute', top: '-10%', left: '-10%', width: '40%', height: '40%', background: 'radial-gradient(circle at center, rgba(99, 102, 241, 0.15) 0%, transparent 70%)', pointerEvents: 'none' }} />
      <div style={{ position: 'absolute', bottom: '-10%', right: '-10%', width: '40%', height: '40%', background: 'radial-gradient(circle at center, rgba(99, 102, 241, 0.15) 0%, transparent 70%)', pointerEvents: 'none' }} />
      
      <main style={{ position: 'relative', zIndex: 10, padding: '0 16px', width: '100%', display: 'flex', justifyContent: 'center' }}>
        <div style={{ 
          background: 'rgba(12, 14, 24, 0.82)', 
          backdropFilter: 'blur(24px)', 
          border: '1px solid rgba(255, 255, 255, 0.08)', 
          maxWidth: '600px', 
          width: '100%', 
          borderRadius: '24px', 
          padding: '48px 32px', 
          boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.5)', 
          display: 'flex', 
          flexDirection: 'column', 
          alignItems: 'center',
          textAlign: 'center'
        }}>
          
          <div style={{ width: '80px', height: '80px', borderRadius: '50%', display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(255, 255, 255, 0.05)', border: '1px solid rgba(255, 255, 255, 0.1)', marginBottom: '32px' }}>
            <span className="material-symbols-outlined" style={{ color: 'var(--color-primary)', fontSize: '40px' }}>{STEPS[step].icon}</span>
          </div>
          
          <div key={step} style={{ animation: `slideIn${slideDir === "right" ? "Right" : "Left"} 0.4s ease-out both`, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '16px', minHeight: '120px' }}>
            <h2 style={{ fontSize: '28px', fontWeight: 800, color: 'var(--color-text-primary)', margin: 0, letterSpacing: '-0.02em' }}>{STEPS[step].title}</h2>
            <p style={{ fontSize: '16px', color: 'var(--color-text-secondary)', margin: 0, lineHeight: 1.6, maxWidth: '80%' }}>{STEPS[step].desc}</p>
          </div>

          <div style={{ display: 'flex', gap: '12px', margin: '40px 0' }}>
            {STEPS.map((_, i) => (
              <div key={i} style={{ 
                width: '12px', height: '12px', borderRadius: '50%', 
                background: i === step ? 'var(--color-accent)' : i < step ? 'var(--color-success)' : 'rgba(255, 255, 255, 0.1)',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
                transition: 'all 0.3s'
              }}>
                {i < step && <span className="material-symbols-outlined" style={{ fontSize: '8px', color: 'white' }}>check</span>}
              </div>
            ))}
          </div>

          <div style={{ display: 'flex', gap: '16px', width: '100%', maxWidth: '300px', flexDirection: 'column' }}>
            {step < STEPS.length - 1 ? (
              <button 
                onClick={goNext}
                style={{ width: '100%', padding: '16px', borderRadius: '12px', background: 'var(--color-primary-container)', color: 'white', border: 'none', fontSize: '16px', fontWeight: 700, cursor: 'pointer', boxShadow: '0 10px 25px -5px rgba(99, 102, 241, 0.4)', transition: 'all 0.2s' }}
              >
                {step === 0 ? "Get Started" : "Next"}
              </button>
            ) : (
              <button 
                onClick={async () => { try { await invoke("set_first_run_complete"); } catch {} window.location.reload(); }}
                style={{ width: '100%', padding: '16px', borderRadius: '12px', background: 'var(--color-success)', color: 'var(--color-bg-dark)', border: 'none', fontSize: '16px', fontWeight: 700, cursor: 'pointer', boxShadow: '0 10px 25px -5px rgba(16, 185, 129, 0.4)', transition: 'all 0.2s' }}
              >
                Start Messaging
              </button>
            )}
            {step > 0 && (
              <button 
                onClick={goBack}
                style={{ width: '100%', padding: '12px', borderRadius: '12px', background: 'transparent', color: 'var(--color-text-muted)', border: '1px solid rgba(255,255,255,0.1)', fontSize: '14px', fontWeight: 600, cursor: 'pointer', transition: 'all 0.2s' }}
              >
                Back
              </button>
            )}
          </div>

          <div style={{ marginTop: '32px', fontFamily: 'var(--font-mono)', fontSize: '11px', color: 'rgba(255, 255, 255, 0.3)' }}>
            Ed25519 · X25519 · XChaCha20-Poly1305
          </div>
        </div>
      </main>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
