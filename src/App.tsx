import { useState, useEffect } from "react";
import "./styles/tokens.css";
import "./styles/theme.css";
import "./styles/animations.css";
import "./styles/reset.css";
import "./styles/layout.css";
import "./styles/components.css";

import { M2MProvider, useM2M } from "./context/M2MContext";
import { ErrorBoundary } from "./components/ErrorBoundary";
import ShortcutHelp from "./components/ShortcutHelp";
import SetupView from "./views/SetupView";
import VaultView from "./views/VaultView";
import HubView from "./views/HubView";
import ChatView from "./views/ChatView";
import SettingsView from "./views/SettingsView";

function AppInner() {
  const { view } = useM2M();
  const [helpOpen, setHelpOpen] = useState(false);

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
    </>
  );
}

function App() {
  return (
    <M2MProvider>
      <AppInner />
    </M2MProvider>
  );
}

export default App;
