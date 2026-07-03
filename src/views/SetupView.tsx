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

  useEffect(() => {
    invoke<boolean>("is_first_run")
      .then((first) => {
        setIsFirstRun(first);
        if (!first) setStep(3);
      })
      .catch(() => {})
      .finally(() => setTimeout(() => setLoading(false), 2200));
  }, []);

  const goNext = () => { if (step < STEPS.length - 1) setStep(s => s + 1); };
  const goBack = () => { if (step > 0) setStep(s => s - 1); };

  if (loading) {
    return (
      <div className="flex w-full min-h-screen items-center justify-center relative overflow-hidden bg-background">
        <main className="relative z-10 px-gutter w-full flex justify-center">
          <div className="flex flex-col items-center text-center animate-in fade-in zoom-in duration-700">
            <div className="w-20 h-20 rounded-full flex items-center justify-center bg-white/5 border border-white/10 mb-xl animate-pulse">
              <span className="material-symbols-outlined text-primary text-4xl">vpn_key</span>
            </div>
            <h2 className="font-headline-2xl text-headline-2xl font-bold text-text-primary mb-sm">Initializing Secure Enclave</h2>
            <p className="font-body-md text-text-secondary mb-xl">Generating Ed25519 identity keys.<br />They never leave your device.</p>
            <div className="flex gap-2">
              <span className="w-1.5 h-1.5 rounded-full bg-primary animate-bounce"></span>
              <span className="w-1.5 h-1.5 rounded-full bg-primary animate-bounce" style={{ animationDelay: '100ms' }}></span>
              <span className="w-1.5 h-1.5 rounded-full bg-primary animate-bounce" style={{ animationDelay: '200ms' }}></span>
            </div>
            <div className="mt-2xl font-mono-label text-label-xs bg-white/5 px-2 py-1 rounded-md text-text-muted border border-white/5">
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
      <div className="flex w-full min-h-screen items-center justify-center relative overflow-hidden bg-background">
        <div className="w-20 h-20 rounded-full flex items-center justify-center bg-white/5 border border-white/10 animate-pulse">
          <span className="material-symbols-outlined text-primary text-4xl">lock</span>
        </div>
      </div>
    );
  }

  return (
    <div className="flex w-full min-h-screen items-center justify-center relative overflow-hidden bg-background">
      <main className="relative z-10 px-gutter w-full flex justify-center">
        <div className="premium-glass-card rounded-3xl max-w-[600px] w-full py-3xl px-2xl flex flex-col items-center text-center relative group">
          <div className="absolute top-0 inset-x-0 h-[1px] bg-gradient-to-r from-transparent via-primary/50 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-1000"></div>
          
          <div className="w-20 h-20 rounded-full flex items-center justify-center bg-white/5 border border-white/10 mb-2xl transition-all duration-300 transform scale-100 hover:scale-110">
            <span className="material-symbols-outlined text-primary text-4xl">{STEPS[step].icon}</span>
          </div>
          
          <div key={step} className="flex flex-col items-center gap-md min-h-[120px] animate-in fade-in slide-in-from-bottom-4 duration-500">
            <h2 className="font-headline-3xl text-headline-3xl font-bold text-text-primary m-0 tracking-tight">{STEPS[step].title}</h2>
            <p className="font-body-lg text-text-secondary m-0 leading-relaxed max-w-[80%]">{STEPS[step].desc}</p>
          </div>

          <div className="flex gap-3 my-2xl">
            {STEPS.map((_, i) => (
              <div key={i} className={`w-3 h-3 rounded-full flex items-center justify-center transition-all duration-300 ${i === step ? 'bg-primary scale-125' : i < step ? 'bg-tertiary' : 'bg-white/10'}`}>
                {i < step && <span className="material-symbols-outlined text-[8px] text-black font-bold">check</span>}
              </div>
            ))}
          </div>

          <div className="flex flex-col gap-lg w-full max-w-[300px]">
            {step < STEPS.length - 1 ? (
              <button 
                onClick={goNext}
                className="premium-btn w-full py-md rounded-xl bg-gradient-to-r from-primary-container to-inverse-primary text-on-primary-container font-headline-2xl text-headline-2xl font-bold hover:brightness-125 transition-all duration-300 shadow-[0_0_20px_rgba(99,102,241,0.2)] hover:shadow-[0_0_30px_rgba(99,102,241,0.5)] group/btn"
              >
                <div className="absolute inset-0 bg-white opacity-0 group-hover/btn:opacity-[0.03] transition-opacity"></div>
                <span className="relative z-10">{step === 0 ? "Get Started" : "Next"}</span>
              </button>
            ) : (
              <button 
                onClick={async () => { try { await invoke("set_first_run_complete"); window.location.reload(); } catch (err: any) { alert(typeof err === "string" ? err : "Failed to finalize setup"); } }}
                className="premium-btn w-full py-md rounded-xl bg-gradient-to-r from-tertiary-container to-tertiary text-on-tertiary-container font-headline-2xl text-headline-2xl font-bold hover:brightness-125 transition-all duration-300 shadow-[0_0_20px_rgba(16,185,129,0.2)] hover:shadow-[0_0_30px_rgba(16,185,129,0.5)] group/btn"
              >
                <div className="absolute inset-0 bg-white opacity-0 group-hover/btn:opacity-[0.03] transition-opacity"></div>
                <span className="relative z-10">Start Messaging</span>
              </button>
            )}
            {step > 0 && (
              <button 
                onClick={goBack}
                className="w-full py-sm rounded-xl bg-transparent text-text-muted border border-white/10 font-label-sm text-label-sm font-semibold hover:text-white hover:bg-white/5 active:scale-95 transition-all"
              >
                Back
              </button>
            )}
          </div>

          <div className="mt-2xl font-mono-label text-[10px] text-white/30 uppercase tracking-widest">
            Ed25519 · X25519 · XChaCha20-Poly1305
          </div>
        </div>
      </main>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
