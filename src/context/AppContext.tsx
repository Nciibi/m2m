import { createContext, useContext, useState, useEffect, useRef, useCallback, useMemo, ReactNode } from "react";
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
  //
  // REMOVED. This effect wrote `data-theme` from the OS preference, for the
  // app's whole lifetime, with no knowledge of the user's explicit choice. So a
  // user who selected the light theme and then let the OS flip to dark at
  // sunset had their selection silently overwritten — and because `ThemeContext`
  // still held `theme: "light"` while the DOM said otherwise, the Settings
  // toggle showed "light" selected while the app rendered dark.
  //
  // `ThemeContext` is the single owner of `data-theme`: it resolves
  // "system" against the OS and subscribes to the media query itself, so the
  // two can no longer disagree.

  // Global keyboard shortcuts
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // `Modal` handles Escape on `document`, which fires *before* this
      // `window` listener during bubbling. Without this check, dismissing the
      // fingerprint-verification dialog in ChatView also navigated to the hub —
      // the user closed the dialog and was thrown out of the conversation.
      // A modal is the innermost keyboard scope, so it wins.
      if (document.querySelector('[role="dialog"]') !== null) return;
      if (e.key === "Escape" && view === "chat") { e.preventDefault(); setView("hub"); }
      if ((e.ctrlKey || e.metaKey) && e.key === ",") { e.preventDefault(); setView("settings"); }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [view, setView]);

  // Memoized: a fresh object literal on every render re-renders every
  // `useApp()` consumer even when nothing it uses has changed. `addToast` in
  // particular is called from all over the app, so a toast firing would
  // otherwise re-render the entire view tree.
  const value = useMemo<AppContextValue>(() => ({
    view, setView,
    toasts, addToast, removeToast,
    identity, vaultInitialized, vaultUnlocked, refreshVault,
  }), [view, setView, toasts, addToast, removeToast, identity, vaultInitialized,
       vaultUnlocked, refreshVault]);

  return (
    <AppContext.Provider value={value}>
      {children}
    </AppContext.Provider>
  );
}
