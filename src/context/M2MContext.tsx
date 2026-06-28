import { ReactNode } from "react";
import { AppProvider, ViewName, useApp } from "./AppContext";
import { VaultProvider, useVault } from "./VaultContext";
import { ChatProvider, useChat } from "./ChatContext";
import { SettingsProvider, useSettings } from "./SettingsContext";

/**
 * Composite provider that wires up all sub-contexts in the correct order.
 *
 * Nesting order (outer → inner):
 *   AppProvider — navigation, toast, identity, theme
 *   VaultProvider — vault unlock
 *   SettingsProvider — network, STUN, diagnostics
 *   ChatProvider — connection, messages, conversations, invites
 *
 * Inner providers can call hooks from outer providers.
 */
export function M2MProvider({ children }: { children: ReactNode }) {
  return (
    <AppProvider>
      <VaultProvider>
        <SettingsProvider>
          <ChatProvider>
            {children}
          </ChatProvider>
        </SettingsProvider>
      </VaultProvider>
    </AppProvider>
  );
}

/**
 * Legacy compatibility hook — merges all sub-contexts into a single object.
 *
 * This exists so existing views continue to work without changes.
 * NEW views should import the specific context hooks directly:
 *
 *   - useApp()       — navigation, toast, identity
 *   - useVault()     — vault unlock
 *   - useChat()      — connection, messages, conversations
 *   - useSettings()  — network, STUN, diagnostics
 *
 * @deprecated Import from the specific context hooks instead.
 */
export function useM2M() {
  const app = useApp();
  // We rely on the context hierarchy — inner contexts call useApp() internally,
  // so the merged result combines everything via the React tree.
  //
  // For migration, each view should switch to the specific hook one at a time.
  // This backward-compat shim will be removed once all views are migrated.
  try {
    const vault = useVault();
    const settings = useSettings();
    const chat = useChat();
    return { ...app, ...vault, ...settings, ...chat } as any;
  } catch {
    return app as any;
  }
}

export type { ViewName };
