import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, ToastContainer } from "../components/ui";
import { KeyIcon, CheckIcon, LockIcon } from "../components/ui/Icons";
import { useApp } from "../context/AppContext";

interface StepData {
  title: string;
  desc: string;
  iconEmoji: string;
}

const STEPS: StepData[] = [
  { title: "Welcome to M2M", desc: "A private, end-to-end encrypted messenger. No servers, no accounts, no tracking.", iconEmoji: "🚀" },
  { title: "Your Identity is Local", desc: "Your keys are generated on this device and never leave it.", iconEmoji: "🔑" },
  { title: "End-to-End Encrypted", desc: "Messages use X3DH + Double Ratchet (Signal protocol). Ed25519 signing, X25519 key exchange, XChaCha20-Poly1305 encryption.", iconEmoji: "🔒" },
  { title: "Ready to Go!", desc: "Share your invite link with a trusted peer to start chatting. Both sides must generate and share invites.", iconEmoji: "✅" },
];

export default function SetupView() {
  const { toasts, removeToast } = useApp();
  const [loading, setLoading] = useState(true);
  const [step, setStep] = useState(0);
  const [isFirstRun, setIsFirstRun] = useState(false);
  const [slideDir, setSlideDir] = useState<"right" | "left">("right");
  const contentRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke<boolean>("is_first_run")
      .then((first) => {
        setIsFirstRun(first);
        if (!first) {
          setStep(3);
        }
      })
      .catch(() => {})
      .finally(() => {
        setTimeout(() => setLoading(false), 2200);
      });
  }, []);

  const goNext = () => {
    if (step < STEPS.length - 1) {
      setSlideDir("right");
      setStep((s) => s + 1);
    }
  };

  const goBack = () => {
    if (step > 0) {
      setSlideDir("left");
      setStep((s) => s - 1);
    }
  };

  // Loading state
  if (loading) {
    return (
      <div className="app-shell">
        <div className="centered-view">
          <div className="setup-icon setup-icon--loading">
            <div className="setup-icon__container">
              <KeyIcon size={36} color="white" />
              <div className="sonar-ring sonar-ring--1" />
              <div className="sonar-ring sonar-ring--2" />
              <div className="sonar-ring sonar-ring--3" />
            </div>
          </div>
          <h2 className="setup-title">Initializing Secure Enclave</h2>
          <p className="setup-desc">
            Generating Ed25519 identity keys.
            <br />
            They never leave your device.
          </p>
          <div className="loading-dots" role="status" aria-label="Generating identity keys">
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

  // Non-first-run: quick transition to vault
  if (!isFirstRun) {
    return (
      <div className="app-shell">
        <div className="centered-view">
          <div className="setup-icon">
            <div className="setup-icon__container">
              <LockIcon size={36} color="white" />
              <div className="sonar-ring sonar-ring--1" />
            </div>
          </div>
          <h2 className="setup-title">Unlocking</h2>
          <div className="loading-dots" role="status" aria-label="Loading">
            <span /><span /><span />
          </div>
        </div>
        <ToastContainer toasts={toasts} onRemove={removeToast} />
      </div>
    );
  }

  const current = STEPS[step];

  return (
    <div className="app-shell">
      <div className="centered-view">
        <div className="setup-icon">
          <div className="setup-icon__container">
            <span className="setup-emoji">{current.iconEmoji}</span>
          </div>
        </div>

        <div
          key={step}
          ref={contentRef}
          className={`setup-step-content setup-step-content--${slideDir}`}
        >
          <h2 className="setup-title">{current.title}</h2>
          <p className="setup-desc">{current.desc}</p>
        </div>

        {/* Step indicator */}
        <div className="step-indicator" role="tablist" aria-label="Onboarding steps">
          {STEPS.map((_, i) => (
            <div
              key={i}
              className={`step-dot ${i === step ? "step-dot--active" : ""} ${i < step ? "step-dot--done" : ""}`}
              role="tab"
              aria-selected={i === step}
              aria-label={`Step ${i + 1}: ${STEPS[i].title}${i < step ? " (completed)" : ""}`}
            >
              {i < step && <CheckIcon size={12} color="white" />}
            </div>
          ))}
        </div>

        {/* Navigation */}
        <div className="onboarding-actions">
          {step < STEPS.length - 1 ? (
            <Button onClick={goNext}>
              {step === 0 ? "Get Started" : "Next"}
            </Button>
          ) : (
            <Button
              onClick={async () => {
                try {
                  await invoke("set_first_run_complete");
                } catch { /* noop */ }
                window.location.reload();
              }}
            >
              Start Messaging
            </Button>
          )}
          {step > 0 && (
            <Button variant="ghost" onClick={goBack}>
              Back
            </Button>
          )}
        </div>

        <div className="crypto-badge onboarding-badge">
          Ed25519 · X25519 · XChaCha20-Poly1305
        </div>
      </div>
      <ToastContainer toasts={toasts} onRemove={removeToast} />
    </div>
  );
}
