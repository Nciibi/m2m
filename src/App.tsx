import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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

/** Active capture tools reported by the backend monitor (empty = clear). */
function CaptureWarningBanner({ active }: { active: string[] }) {
  if (active.length === 0) return null;
  return (
    <div
      role="alert"
      className="capture-warning"
      style={{
        position: "fixed", top: 0, left: 0, right: 0, zIndex: 9998,
        background: "var(--color-danger, #dc2626)", color: "#fff",
        padding: "6px 14px", fontSize: 13, textAlign: "center",
      }}
    >
      ⚠ Screen capture software detected: {active.join(", ")} — your screen may be recorded.
    </div>
  );
}

function AppInner() {
  const { view } = useApp();
  const [helpOpen, setHelpOpen] = useState(false);
  const [captureWarning, setCaptureWarning] = useState<string[]>([]);
  const { securityConfig } = useSettings();

  // Focus-loss blur (off unless enabled in security settings).
  const blurred = useFocusBlur(securityConfig?.blur_on_focus_loss ?? false);

  // On mount / webview reload: ask the backend to re-apply the persisted
  // security config so capture protection never silently drops after a
  // webview recreation. Also subscribe to capture-software warnings.
  useEffect(() => {
    invoke("reapply_security_config").catch(() => { /* backend may not be ready yet */ });

    const unlisten = listen<{ active: string[] }>("m2m://capture-warning", (event) => {
      setCaptureWarning(event.payload.active ?? []);
    }).catch(() => () => {});

    return () => { unlisten.then((fn) => fn()).catch(() => {}); };
  }, []);

  useIdleDetection({
    timeoutSecs: securityConfig?.idle_lock_secs ?? 0,
    onIdle: () => { invoke("lock_vault").catch(() => {}); },
  });

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "?" && !e.ctrlKey && !e.metaKey && !e.altKey && e.target instanceof Element && e.target.tagName !== 'INPUT' && e.target.tagName !== 'TEXTAREA') {
        setHelpOpen((prev) => !prev);
      }
    };
    
    // Premium Mouse Spotlight Effect
    const handleMouseMove = (e: MouseEvent) => {
      document.documentElement.style.setProperty('--cursor-x', `${e.clientX}px`);
      document.documentElement.style.setProperty('--cursor-y', `${e.clientY}px`);
    };

    window.addEventListener("keydown", handler);
    window.addEventListener("mousemove", handleMouseMove);
    
    return () => {
      window.removeEventListener("keydown", handler);
      window.removeEventListener("mousemove", handleMouseMove);
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
    <>
      <ErrorBoundary name={view}>
        <div key={view} className="view-fade">
          {viewComponent}
        </div>
      </ErrorBoundary>
      <ShortcutHelp open={helpOpen} onClose={() => setHelpOpen(false)} />
    </>
  );
}

function App() {
  return (
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
  );
}

export default App;
