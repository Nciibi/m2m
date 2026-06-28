import { createContext, useContext, useCallback, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useApp } from "./AppContext";

interface VaultContextValue {
  handleUnlockVault: (passphrase: string) => Promise<void>;
}

const VaultContext = createContext<VaultContextValue | null>(null);

export function useVault(): VaultContextValue {
  const ctx = useContext(VaultContext);
  if (!ctx) throw new Error("useVault() must be used within <VaultProvider>");
  return ctx;
}

export function VaultProvider({ children }: { children: ReactNode }) {
  const { setView } = useApp();

  const handleUnlockVault = useCallback(async (passphrase: string) => {
    await invoke("unlock_vault", { passphrase });
    // After unlock, the vault state is persisted in storage and will be
    // re-read by AppProvider on the next init. We transition to hub to
    // let the view system refresh identity state.
    setView("hub");
  }, [setView]);

  return (
    <VaultContext.Provider value={{ handleUnlockVault }}>
      {children}
    </VaultContext.Provider>
  );
}
