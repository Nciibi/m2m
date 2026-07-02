import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
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
import ShortcutHelp from "./components/ShortcutHelp";
import SetupView from "./views/SetupView";
import VaultView from "./views/VaultView";
import HubView from "./views/HubView";
import ChatView from "./views/ChatView";
import SettingsView from "./views/SettingsView";

function AppInner() {
  const { view } = useApp();
  const [helpOpen, setHelpOpen] = useState(false);
  const { securityConfig } = useSettings();

  // Idle detection for auto-lock
  useIdleDetection({
    timeoutSecs: securityConfig?.idle_lock_secs ?? 0,
    onIdle: () => { invoke("lock_vault").catch(() => {}); },
  });

  // Global keyboard shortcut: ? opens help modal
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "?" && !e.ctrlKey && !e.metaKey && !e.altKey) {
        setHelpOpen((prev) => !prev);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const viewComponent = (() => {
    switch (view) {
      case "setup": return <SetupView />;
      case "vault": return <VaultView />;
      case "settings": return <SettingsView />;
      case "hub": return <HubView />;
      case "chat": return <ChatView />;
      default: return <SetupView />;
    }
  })();

  return (
    <>
      <ErrorBoundary name={view}>
        {viewComponent}
      </ErrorBoundary>
      <ShortcutHelp open={helpOpen} onClose={() => setHelpOpen(false)} />
      <UpdateBanner />
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
