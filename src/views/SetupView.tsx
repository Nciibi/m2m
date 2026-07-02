import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, ToastContainer } from "../components/ui";
import { KeyIcon, CheckIcon, LockIcon } from "../components/ui/Icons";
import { useApp } from "../context/AppContext";

const STEPS = [
  { title: "Welcome to M2M", desc: "A private, end-to-end encrypted messenger. No servers, no accounts, no tracking.", icon: "🚀" },
  { title: "Your Identity is Local", desc: "Your keys are generated on this device and never leave it.", icon: "🔑" },
  { title: "End-to-End Encrypted", desc: "Messages use X3DH + Double Ratchet (Signal protocol). Ed25519 signing, X25519 key exchange, XChaCha20-Poly1305 encryption.", icon: "🔒" },
  { title: "Ready to Go!", desc: "Share your invite link with a trusted peer to start chatting. Both sides must generate and share invites.", icon: "✅" },
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
      <div className="app-shell">
        <div className="centered-view">
          <div className="setup-icon__container">
            <KeyIcon size={36} color="white" />
            <div className="sonar-ring sonar-ring--1" />
            <div className="sonar-ring sonar-ring--2" />
            <div className="sonar-ring sonar-ring--3" />
          </div>
          <h2 className="setup-title">Initializing Secure Enclave</h2>
          <p className="setup-desc">Generating Ed25519 identity keys.<br />They never leave your device.</p>
          <div className="loading-dots" role="status" aria-label="Generating identity keys">
            <span /><span /><span />
          </div>
          <div className="crypto-badge">Ed25519 · X25519 · XChaCha20-Poly1305</div>
        </div>
        <ToastContainer toasts={toasts} onRemove={removeToast} />
      </div>
    );
  }

  if (!isFirstRun) {
    return (
      <div className="app-shell">
        <div className="centered-view">
          <div className="setup-icon__container">
            <LockIcon size={36} color="white" />
            <div className="sonar-ring sonar-ring--1" />
          </div>
          <h2 className="setup-title">Initializing Secure Enclave</h2>
          <div className="loading-dots" role="status"><span /><span /><span /></div>
        </div>
        <ToastContainer toasts={toasts} onRemove={removeToast} />
      </div>
    );
  }

  return (
    <div className="app-shell">
      <div className="centered-view">
        <div className="setup-icon__container">
          <span className="setup-emoji">{STEPS[step].icon}</span>
        </div>
        <div key={step} className={`setup-step-content setup-step-content--${slideDir}`}>
          <h2 className="setup-title">{STEPS[step].title}</h2>
          <p className="setup-desc">{STEPS[step].desc}</p>
        </div>
        <div className="step-indicator" role="tablist" aria-label="Onboarding steps">
          {STEPS.map((_, i) => (
            <div key={i} className={`step-dot ${i === step ? "step-dot--active" : ""} ${i < step ? "step-dot--done" : ""}`} role="tab" aria-selected={i === step}>
              {i < step && <CheckIcon size={12} color="white" />}
            </div>
          ))}
        </div>
        <div className="onboarding-actions">
          {step < STEPS.length - 1 ? (
            <Button onClick={goNext}>{step === 0 ? "Get Started" : "Next"}</Button>
          ) : (
            <Button onClick={async () => { try { await invoke("set_first_run_complete"); } catch {} window.location.reload(); }}>
              Start Messaging
            </Button>
          )}
          {step > 0 && <Button variant="ghost" onClick={goBack}>Back</Button>}
        </div>
        <div className="crypto-badge onboarding-badge">Ed25519 · X25519 · XChaCha20-Poly1305</div>
      </div>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
