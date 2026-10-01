import { createContext, useContext, useCallback, useMemo, ReactNode } from "react";
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
  // `refreshVault`, not `setView`.
  //
  // `AppContext.setView` is guarded: it only navigates when `unlockedRef.current`
  // is true (or the target is the vault/setup screen). That ref is written *only*
  // by `refreshVault` and the `m2m://vault-locked` handler, and `refreshVault` ran
  // for the last time during the mount effect. So after a successful
  // `unlock_vault`, calling `setView("hub")` hit the guard with
  // `unlockedRef.current === false` and was **silently dropped**: the spinner
  // stopped, no error appeared, and the user was left on the unlock screen having
  // typed a correct passphrase, with `identity` still `null` app-wide.
  //
  // `refreshVault` sets the ref, the `vaultUnlocked`/`vaultInitialized` state and
  // `identity` from the backend's own answer, and navigates to `hub` itself.
  // Routing through it also means this path cannot disagree with the status the
  // backend reports.
  const { refreshVault } = useApp();

  const handleUnlockVault = useCallback(async (passphrase: string) => {
    await invoke("unlock_vault", { passphrase });
    await refreshVault();
  }, [refreshVault]);

  const value = useMemo<VaultContextValue>(() => ({ handleUnlockVault }), [handleUnlockVault]);

  return (
    <VaultContext.Provider value={value}>
      {children}
    </VaultContext.Provider>
  );
}
