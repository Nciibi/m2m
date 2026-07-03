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
import { UpdateBanner } from "./components/ui";
import SetupView from "./views/SetupView";
import VaultView from "./views/VaultView";
import HubView from "./views/HubView";
import ChatView from "./views/ChatView";
import GroupChatView from "./views/GroupChatView";
import SettingsView from "./views/SettingsView";

function AppInner() {
  const { view } = useApp();
  const [helpOpen, setHelpOpen] = useState(false);
  const { securityConfig } = useSettings();

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
