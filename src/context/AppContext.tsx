import { createContext, useContext, useState, useEffect, useRef, useCallback, ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
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
  vaultUnlocked: boolean;
  refreshVault: () => Promise<void>;
}

const AppContext = createContext<AppContextValue | null>(null);

export function useApp(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useApp() must be used within <AppProvider>");
  return ctx;
}

export function AppProvider({ children }: { children: ReactNode }) {
  const { toasts, addToast: pushToast, removeToast, clearToasts } = useToast();
  const [view, updateView] = useState<ViewName>("setup");
  const [vaultUnlocked, setVaultUnlocked] = useState(false);
  const unlockedRef = useRef(false);
  const lockGeneration = useRef(0);
  const setView = useCallback((next: ViewName) => {
    if (unlockedRef.current || next === "vault" || next === "setup") updateView(next);
  }, []);
  const addToast = useCallback((...args: Parameters<typeof pushToast>) => {
    pushToast(...args);
  }, [pushToast]);
  const [identity, setIdentity] = useState<IdentityInfo | null>(null);
  const [vaultInitialized, setVaultInitialized] = useState(false);

  const refreshVault = useCallback(async () => {
    const generation = lockGeneration.current;
    const status = await invoke<VaultStatus>("get_vault_status");
    const info = status.unlocked ? await invoke<IdentityInfo>("get_identity") : null;
    if (generation !== lockGeneration.current) return;
    unlockedRef.current = status.unlocked;
    setVaultUnlocked(status.unlocked);
    setVaultInitialized(status.initialized);
    setIdentity(info);
    updateView(status.unlocked ? "hub" : "vault");
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    async function initialize() {
      try {
        const stop = await listen("m2m://vault-locked", () => {
          if (disposed) return;
          lockGeneration.current += 1;
          unlockedRef.current = false;
          setVaultUnlocked(false);
          setIdentity(null);
          clearToasts();
          updateView("vault");
        });
        if (disposed) { stop(); return; }
        unlisten = stop;
        await invoke("init_identity");
        if (!disposed) await refreshVault();
      } catch (err) {
        console.error("Init failed:", err);
      }
    }
    void initialize();
    return () => { disposed = true; unlisten?.(); };
  }, [clearToasts, refreshVault]);

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
  }, [view, setView]);

  return (
    <AppContext.Provider value={{
      view, setView,
      toasts, addToast, removeToast,
      identity, vaultInitialized, vaultUnlocked, refreshVault,
    }}>
      {children}
    </AppContext.Provider>
  );
}
