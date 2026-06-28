import { createContext, useContext, ReactNode } from "react";
import { useM2MState, ViewName } from "../hooks/useM2MState";
import type {
  IdentityInfo, ConnectionInfo, ChatMessage, FileRequest,
  ConversationEntry, NetworkSettings, StunConfig, NatTypeInfo,
  Toast,
} from "../types";

/** All state and actions exposed by the M2M context. */
export interface M2MContextValue {
  // Toast
  toasts: Toast[];
  addToast: (msg: string, variant?: string, duration?: number) => void;
  removeToast: (id: string) => void;
  // View navigation
  view: ViewName;
  setView: (v: ViewName) => void;
  // Identity
  identity: IdentityInfo | null;
  // Connection
  connection: ConnectionInfo | null;
  isConnecting: boolean;
  // Messages
  messages: ChatMessage[];
  fileRequests: FileRequest[];
  // Vault
  vaultInitialized: boolean;
  // Settings data
  networkSettings: NetworkSettings | null;
  publicIp: string | null;
  stunLoading: boolean;
  networkDiagnostics: NatTypeInfo | null;
  stunConfig: StunConfig | null;
  stunServerInput: string;
  privateMode: boolean;
  connectivityResult: any;
  // Conversations
  conversations: ConversationEntry[];
  activeConversationId: string | null;
  // Naming
  inviteToConnect: string;
  inviteValid: boolean;
  namingMyName: string;
  namingTheirName: string;
  // Invite
  generatedInvite: string;
  // Retention
  retentionPolicy: string;
  retentionDuration: string;
  // Mutators
  setStunServerInput: (v: string) => void;
  setInviteToConnect: (v: string) => void;
  setNamingMyName: (v: string) => void;
  setNamingTheirName: (v: string) => void;
  setRetentionPolicy: (v: string) => void;
  setRetentionDuration: (v: string) => void;
  handleUnlockVault: (passphrase: string) => Promise<void>;
  handleSendMessage: (content: string) => Promise<void>;
  handleVerify: () => Promise<void>;
  handleDisconnect: () => Promise<void>;
  handleSendFile: () => Promise<void>;
  handleExportConversation: () => Promise<void>;
  handleSetRetention: (policy: string, durationSecs: number | null) => Promise<void>;
  openSettings: () => Promise<void>;
  handleGenerateInvite: () => Promise<void>;
  copyInvite: () => void;
  handleConnect: () => Promise<void>;
  handleOpenChat: (conv: ConversationEntry) => Promise<void>;
  handleStunDiscover: () => Promise<void>;
  handleAddStunServer: () => Promise<void>;
  handleRemoveStunServer: (idx: number) => Promise<void>;
  handleResetStunDefaults: () => Promise<void>;
  handlePrivateModeToggle: () => Promise<void>;
  handleConnectivityCheck: () => Promise<void>;
  handleTorToggle: () => Promise<void>;
  handleDeleteConversation: () => void;
}

const M2MContext = createContext<M2MContextValue | null>(null);

/** Hook to consume the M2M context. */
export function useM2M(): M2MContextValue {
  const ctx = useContext(M2MContext);
  if (!ctx) throw new Error("useM2M() must be used within <M2MProvider>");
  return ctx;
}

export function M2MProvider({ children }: { children: ReactNode }) {
  const state = useM2MState();

  // We return the full state as the context value.
  // TypeScript will check it matches M2MContextValue.
  return (
    <M2MContext.Provider value={state as unknown as M2MContextValue}>
      {children}
    </M2MContext.Provider>
  );
}
