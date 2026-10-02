import { useState, useEffect, useCallback } from "react";
import { I18nProvider } from "./i18n/I18nContext";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { errorMessage, evictionNoticeText } from "./utils";
import { asCaptureWarning, asSecurityError, asStorageEvicted, asNavigate } from "./events";
import "./styles/tokens.css";
import "./styles/theme.css";
import "./styles/animations.css";
import "./styles/reset.css";
import "./styles/layout.css";
import "./styles/components/index.css";

import { AppProvider, useApp } from "./context/AppContext";
import { VaultProvider } from "./context/VaultContext";
import { ChatProvider } from "./context/ChatContext";
import { SettingsProvider, useSettings } from "./context/SettingsContext";
import { ThemeProvider } from "./context/ThemeContext";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { useIdleDetection } from "./hooks/useIdleDetection";
import { useFocusBlur } from "./hooks/useFocusBlur";
import ShortcutHelp from "./components/ShortcutHelp";
import SetupView from "./views/SetupView";
import VaultView from "./views/VaultView";
import HubView from "./views/HubView";
import ChatView from "./views/ChatView";
import GroupChatView from "./views/GroupChatView";
import SettingsView from "./views/SettingsView";

/**
 * A standing security condition the user must be able to see: either active
 * capture tooling, or a security control that failed to apply. `role="alert"`
 * because both are assertive — this is not something to notice in passing.
 */
function SecurityBanner({
  body,
  onDismiss,
}: {
  body: string | null;
  onDismiss?: () => void;
}) {
  if (!body) return null;
  return (
    <div
      role="alert"
      className="capture-warning"
      style={{
        // The z-index belongs to the scale: 9998 was a bare literal sitting
        // between `--z-modal` and `--z-toast`, so reordering the scale could
        // silently put the banner over a modal or under a toast. The colour
        // keeps a `var(--token, <literal>)` fallback on purpose — the one
        // sanctioned exception, so the banner still gets a colour if the
        // stylesheet fails to load.
        position: "fixed", top: 0, left: 0, right: 0, zIndex: "var(--z-banner)",
        background: "var(--color-danger, #dc2626)", color: "var(--color-on-danger, #fff)",
        padding: "6px 14px", fontSize: 13, textAlign: "center",
      }}
    >
      <span>{body}</span>
      {onDismiss && (
        <button
          type="button"
          onClick={onDismiss}
          aria-label="Dismiss notice"
          style={{ marginLeft: 10, color: "inherit", background: "none", border: 0, cursor: "pointer", font: "inherit" }}
        >
          ✕
        </button>
      )}
    </div>
  );
}

function AppInner() {
  const { view, setView } = useApp();
  const [helpOpen, setHelpOpen] = useState(false);
  const [captureWarning, setCaptureWarning] = useState<string[]>([]);
const [captureScanFailed, setCaptureScanFailed] = useState(false);
  const [securityError, setSecurityError] = useState<string | null>(null);
  // Set when the storage cap permanently destroys history, so the user is told
  // rather than finding messages gone with no explanation.
  const [evictionNotice, setEvictionNotice] = useState<string | null>(null);
  const { securityConfig } = useSettings();

  // Focus-loss blur (off unless enabled in security settings).
  const blurred = useFocusBlur(securityConfig?.blur_on_focus_loss ?? false);

  // On mount / webview reload: ask the backend to re-apply the persisted
  // security config so capture protection never silently drops after a
  // webview recreation. Also subscribe to capture-software warnings.
  useEffect(() => {
    invoke("reapply_security_config").catch(() => { /* backend may not be ready yet */ });

    // Validate through `asCaptureWarning` rather than trusting
    // `listen<{active: string[]}>`. The generic is an assertion, not a check: it
    // told the compiler the payload was already the right shape, which is the
    // opposite of what the boundary has to establish. `events.ts` already
    // implements a tested guard for this exact event — length-bounded and
    // control-character-screened, because these strings are rendered into the
    // DOM by `active.join(", ")`. It was simply never called, so the banner
    // accepted an unbounded, unfiltered array from a process-enumeration result
    // — the one place in the app where a peer's software choice becomes UI
    // text.
    const unlisten = listen("m2m://capture-warning", (event) => {
      const payload = asCaptureWarning(event.payload);
      if (!payload) return;  // malformed → drop, never render
      // Both halves are set from the same validated event, so the banner can
      // never show a stale detection list alongside a fresh health state.
      setCaptureWarning(payload.active);
      setCaptureScanFailed(payload.scanFailed);
    }).catch(() => () => {});

    // `m2m://navigate` — the tray menu's "New Conversation" and "Settings"
    // items. This event was emitted from `lib.rs` with a bare JSON string and
    // had **no listener anywhere in `src/`**, so both items showed and focused
    // the window and then did nothing at all: the user landed on whatever view
    // happened to already be open, with no error to explain it.
    //
    // Validated against a closed set rather than passed through — it names the
    // view to switch to, so an unrecognised value must leave us where we are.
    const unlistenNav = listen("m2m://navigate", (event) => {
      const target = asNavigate(event.payload);
      if (!target) {
        console.warn("M2M: dropping malformed m2m://navigate payload");
        return;
      }
      setView(target);
    }).catch(() => () => {});

    // `m2m://security-error` — emitted by the backend when a security control
    // FAILS to apply, e.g. screen-capture protection could not be established.
    // The event and its validator both existed and nothing listened, so the
    // exact case the mechanism was built for — a protection silently not
    // applying — reached the user as complete silence. Surfaced as an
    // assertive banner rather than a toast, because it is a standing condition,
    // not a transient event.
    const unlistenSec = listen("m2m://security-error", (event) => {
      const payload = asSecurityError(event.payload);
      if (!payload) return;  // malformed → drop, never render
      setSecurityError(`${payload.source}: ${payload.message}`);
    }).catch(() => () => {});

    // `m2m://storage-evicted` — the backend hit the storage cap and
    // permanently deleted the oldest messages. Surfaced as a standing notice
    // rather than a toast: a toast disappears, and this needs the user to
    // understand that history is now gone and that raising the cap is how to
    // stop it recurring.
    const unlistenEvict = listen("m2m://storage-evicted", (event) => {
      const payload = asStorageEvicted(event.payload);
      if (!payload) return;  // malformed → drop, never render
      const text = evictionNoticeText(payload);
      if (text) setEvictionNotice(text);
    }).catch(() => () => {});

    return () => {
      unlisten.then((fn) => fn()).catch(() => {});
      unlistenSec.then((fn) => fn()).catch(() => {});
      unlistenEvict.then((fn) => fn()).catch(() => {});
      unlistenNav.then((fn) => fn()).catch(() => {});
    };
    // `setView` is a stable `useCallback` over `[]`, so listing it re-registers
    // nothing — it is here because the `m2m://navigate` handler closes over it,
    // and omitting it would be the one thing that could later make the tray
    // items navigate somewhere stale.
  }, [setView]);

  // Auto-lock on idle.
  //
  // A silent `catch` here meant that if `lock_vault` failed, the user believed
  // their vault had auto-locked and it had not. For a tool whose users may be
  // under physical coercion, "I left it for 5 minutes and it locked itself" is
  // a claim they might act on — so a failure has to be visible, not swallowed.
  const { addToast } = useApp();
  const onIdle = useCallback(() => {
    invoke("lock_vault").catch((err) => {
      console.error("idle auto-lock failed", err);
      addToast("Auto-lock FAILED — lock the vault manually", "error");
    });
  }, [addToast]);

  useIdleDetection({
    timeoutSecs: securityConfig?.idle_lock_secs ?? 0,
    onIdle,
  });

  // Emergency panic wipe (Ctrl+Alt+Shift+W) — only when explicitly armed
  // in Security settings. Wipes all local data and exits, no confirmation.
  useEffect(() => {
    if (!securityConfig?.panic_hotkey_enabled) return;
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.altKey && e.shiftKey && (e.key === "W" || e.key === "w")) {
        e.preventDefault();
        invoke("panic_wipe").catch((err) => {
          // The backend refused (or the wipe itself failed). This was a bare
          // `console.error`, so a user who just pressed the emergency wipe
          // hotkey — mid-incident — saw nothing at all and would conclude they
          // were safe. The catalog already has a string for this case.
          console.error("panic wipe refused:", err);
          addToast(
            "PANIC WIPE FAILED — your data was NOT wiped. " + errorMessage(err),
            "error",
            0,
          );
        });
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [securityConfig?.panic_hotkey_enabled, addToast]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "?" && !e.ctrlKey && !e.metaKey && !e.altKey && e.target instanceof Element && e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
        setHelpOpen((prev) => !prev);
      }
    };
    
    // Mouse spotlight sheen (`--cursor-x` / `--cursor-y`, consumed by
    // animations.css and utilities.css).
    //
    // Throttled to one write per animation frame. These are custom properties
    // on `documentElement`, so every write invalidates style for the whole
    // document; at a 120 Hz pointer rate the unthrottled version forced roughly
    // 240 full style recalculations per second for the lifetime of the app, to
    // feed two radial gradients. Coalescing to rAF keeps the effect visually
    // identical — the gradient is already only painted once per frame — and
    // bounds the cost to the display refresh rate.
    let rafId: number | null = null;
    let pendingX = 0;
    let pendingY = 0;
    const flush = () => {
      rafId = null;
      document.documentElement.style.setProperty('--cursor-x', `${pendingX}px`);
      document.documentElement.style.setProperty('--cursor-y', `${pendingY}px`);
    };
    const handleMouseMove = (e: MouseEvent) => {
      pendingX = e.clientX;
      pendingY = e.clientY;
      if (rafId === null) rafId = requestAnimationFrame(flush);
    };

    window.addEventListener("keydown", handler);
    window.addEventListener("mousemove", handleMouseMove);

    return () => {
      window.removeEventListener("keydown", handler);
      window.removeEventListener("mousemove", handleMouseMove);
      if (rafId !== null) cancelAnimationFrame(rafId);
    };
  }, []);

  const viewComponent = (() => {
    switch (view) {
      case "setup": return <SetupView />;
      case "vault": return <VaultView />;
      case "settings": return <SettingsView />;
      case "hub": return <HubView />;
      case "chat": return <ChatView />;
      case "groups": return <GroupChatView />;
      default: return <SetupView />;
    }
  })();

  return (
    // The blur wrapper covers EVERYTHING, not just the view.
    //
    // It previously wrapped only `viewComponent`, leaving
    // `<CaptureWarningBanner>` and `<ShortcutHelp>` as siblings outside it —
    // so those were never blurred, and the "blur everything on focus loss"
    // guarantee did not actually hold. The class now goes on the outermost
    // element.
    //
    // `inert` (React 19) is applied alongside `aria-hidden`. `aria-hidden`
    // alone removes content from the *accessibility tree* but leaves it
    // focusable and clickable, so a keyboard user could still Tab into blurred
    // content and interact with it. `inert` takes it out of both the a11y tree
    // and the tab order, and the underlying content is only blurred when the
    // window is not focused anyway.
    <div className={blurred ? "security-blur" : undefined} aria-hidden={blurred || undefined} inert={blurred || undefined}>
      <SecurityBanner
        body={
          securityError ??
          evictionNotice ??
          (captureScanFailed
            // A failed scan is reported as its own state. Rendering nothing here
            // would mean the app asserted nothing was detected at the exact
            // moment it had lost the ability to detect.
            ? `⚠ Screen capture detection is not working right now, so this screen may be recorded without warning.${captureWarning.length > 0 ? ` Currently flagged: ${captureWarning.join(", ")}.` : ""}`
            : captureWarning.length > 0
              ? `⚠ Screen capture software detected: ${captureWarning.join(", ")} — your screen may be recorded.`
              : null)
        }
        onDismiss={evictionNotice ? () => setEvictionNotice(null) : undefined}
      />
      <ErrorBoundary name={view}>
        <div className="view-fade" key={view}>
          {viewComponent}
        </div>
      </ErrorBoundary>
      <ShortcutHelp open={helpOpen} onClose={() => setHelpOpen(false)} />
    </div>
  );
}

function App() {
  return (
    // I18nProvider is outermost: it has no dependencies, and every other
    // provider below calls `useT()`.
    <I18nProvider>
      <AppProvider>
        <VaultProvider>
          <SettingsProvider>
            <ThemeProvider>
              <ChatProvider>
                <AppInner />
              </ChatProvider>
            </ThemeProvider>
          </SettingsProvider>
        </VaultProvider>
      </AppProvider>
    </I18nProvider>
  );
}

export default App;
