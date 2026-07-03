import { createContext, useContext, useState, useEffect, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useToast } from "../hooks/useToast";
import type { IdentityInfo, VaultStatus } from "../types";
import type { ToastData } from "../components/ui/Toast";

export type ViewName = "setup" | "vault" | "hub" | "chat" | "settings" | "groups";

interface AppContextValue {
  // Navigation
  view: ViewName;
  setView: (v: ViewName) => void;
  // Toast
  toasts: ToastData[];
  addToast: (msg: string, type?: ToastData["type"], duration?: number) => void;
  removeToast: (id: string) => void;
  // Identity
  identity: IdentityInfo | null;
  vaultInitialized: boolean;
}

const AppContext = createContext<AppContextValue | null>(null);

export function useApp(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useApp() must be used within <AppProvider>");
  return ctx;
}

export function AppProvider({ children }: { children: ReactNode }) {
  const { toasts, addToast, removeToast } = useToast();
  const [view, setView] = useState<ViewName>("setup");
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const [vaultInitialized, setVaultInitialized] = useState(false);

  // OnInit: check identity
  useEffect(() => {
    async function check() {
      try {
        const info = await invoke<IdentityInfo>("init_identity");
        setIdentity(info);
        if (info.has_identity) {
          const vs = await invoke<VaultStatus>("get_vault_status");
          setVaultInitialized(vs.initialized);
          setView(vs.unlocked ? "hub" : "vault");
        } else {
          setVaultInitialized(false);
          setView("vault");
        }
      } catch (err) {
        console.error("Init failed:", err);
      }
    }
    check();
  }, []);

  // Theme detection
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: light)");
    const update = (e: MediaQueryListEvent | MediaQueryList) => {
      document.documentElement.setAttribute("data-theme", e.matches ? "light" : "dark");
    };
    update(mq);
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  }, []);

  // Global keyboard shortcuts
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape" && view === "chat") { e.preventDefault(); setView("hub"); }
      if ((e.ctrlKey || e.metaKey) && e.key === ",") { e.preventDefault(); setView("settings"); }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [view]);

  return (
    <AppContext.Provider value={{
      view, setView,
      toasts, addToast, removeToast,
      identity, vaultInitialized,
    }}>
      {children}
    </AppContext.Provider>
  );
}
