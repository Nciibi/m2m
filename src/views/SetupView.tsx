import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, ToastContainer } from "../components/ui";
import { KeyIcon, CheckIcon } from "../components/ui/Icons";
import { useApp } from "../context/AppContext";

export default function SetupView() {
  const { toasts, removeToast } = useApp();
  const [step, setStep] = useState(0);
  const [initialized, setInitialized] = useState(false);
  const [isFirstRun, setIsFirstRun] = useState(false);

  useEffect(() => {
    // Check if this is first run by looking for a flag
    invoke<boolean>("is_first_run").then((first) => {
      setIsFirstRun(first);
      if (!first) {
        setInitialized(true);
        setStep(3); // Skip to "ready"
      }
    }).catch(() => {
      setInitialized(true);
    });

    // Auto-advance through initialization
    const timer = setTimeout(() => {
      setInitialized(true);
      setStep(1);
    }, 3000);

    return () => clearTimeout(timer);
  }, []);

  if (!isFirstRun && initialized) {
    // Fast path: existing user, just show loading
    return (
      <div className="app-shell">
        <div className="centered-view">
          <div className="setup-icon">
            <KeyIcon size={36} color="white" />
            <div className="setup-icon__glow" />
          </div>
          <h2 className="centered-view__title centered-view__title--spaced">
            Initializing Secure Enclave
          </h2>
          <p className="centered-view__desc centered-view__desc--spaced">
            Generating Ed25519 identity keys.
            <br />
            They never leave your device.
          </p>
          <div className="loading-dots" role="status" aria-label="Generating keys">
            <span /><span /><span />
          </div>
          <div className="crypto-badge">
            Ed25519 · X25519 · XChaCha20-Poly1305
          </div>
        </div>
        <ToastContainer toasts={toasts} onRemove={removeToast} />
      </div>
    );
  }

  // Onboarding wizard for first-run users
  const steps = [
    {
      title: "Welcome to M2M",
      desc: "A private, end-to-end encrypted messenger. No servers, no accounts, no tracking.",
      icon: "🚀",
    },
    {
      title: "Your Identity is Local",
      desc: "Your keys are generated on this device and never leave it. Not even to us — because there is no us.",
      icon: "🔑",
    },
    {
      title: "End-to-End Encrypted",
      desc: "Messages use X3DH + Double Ratchet (Signal protocol). Ed25519 signing, X25519 key exchange, XChaCha20-Poly1305 encryption.",
      icon: "🔒",
    },
    {
      title: "Ready to Go!",
      desc: "Share your invite link with a trusted peer to start chatting. Both sides must generate and share invites.",
      icon: "✅",
    },
  ];

  const current = steps[Math.min(step, steps.length - 1)];

  return (
    <div className="app-shell">
      <div className="centered-view">
        <div className="setup-icon" style={{ fontSize: '2.5rem' }}>
          {current.icon}
          <div className="setup-icon__glow" />
        </div>

        <h2 className="centered-view__title centered-view__title--spaced">
          {current.title}
        </h2>

        <p className="centered-view__desc centered-view__desc--spaced">
          {current.desc}
        </p>

        {/* Step indicators */}
        <div className="onboarding-steps">
          {steps.map((_, i) => (
            <div
              key={i}
              className={`onboarding-dot ${i === step ? 'onboarding-dot--active' : ''} ${i < step ? 'onboarding-dot--done' : ''}`}
            >
              {i < step ? <CheckIcon size={12} /> : null}
            </div>
          ))}
        </div>

        <div className="onboarding-actions" style={{ marginTop: 'var(--space-2xl)', display: 'flex', gap: 'var(--space-md)' }}>
          {step < steps.length - 1 ? (
            <Button onClick={() => setStep((s) => Math.min(s + 1, steps.length - 1))}>
              {step === 0 ? "Get Started" : "Next"}
            </Button>
          ) : (
            <Button onClick={async () => {
              // Mark first run as complete
              try { await invoke("set_first_run_complete"); } catch { /* noop */ }
              // Reload to proceed to vault
              window.location.reload();
            }}>
              Start Messaging
            </Button>
          )}
          {step > 0 && step < steps.length - 1 && (
            <Button variant="secondary" onClick={() => setStep((s) => Math.max(s - 1, 0))}>
              Back
            </Button>
          )}
        </div>

        <div className="crypto-badge" style={{ marginTop: 'var(--space-lg)' }}>
          Ed25519 · X25519 · XChaCha20-Poly1305 · Double Ratchet
        </div>
      </div>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
