import "./styles/tokens.css";
import "./styles/theme.css";
import "./styles/animations.css";
import "./styles/reset.css";
import "./styles/layout.css";
import "./styles/components.css";

import { useM2MState } from "./hooks/useM2MState";
import SetupView from "./views/SetupView";
import VaultView from "./views/VaultView";
import HubView from "./views/HubView";
import ChatView from "./views/ChatView";
import SettingsView from "./views/SettingsView";

function App() {
  const state = useM2MState();

  switch (state.view) {
    case "setup":
      return <SetupView toasts={state.toasts} removeToast={state.removeToast} />;
    case "vault":
      return (
        <VaultView
          vaultInitialized={state.vaultInitialized}
          onUnlock={state.handleUnlockVault}
          toasts={state.toasts}
          removeToast={state.removeToast}
        />
      );
    case "settings":
      return (
        <SettingsView
          identity={state.identity}
          networkSettings={state.networkSettings}
          publicIp={state.publicIp}
          stunLoading={state.stunLoading}
          networkDiagnostics={state.networkDiagnostics}
          stunConfig={state.stunConfig}
          stunServerInput={state.stunServerInput}
          privateMode={state.privateMode}
          connectivityResult={state.connectivityResult}
          toasts={state.toasts}
          removeToast={state.removeToast}
          onBackToHub={() => state.setView("hub")}
          onStunDiscover={state.handleStunDiscover}
          onAddStunServer={state.handleAddStunServer}
          onRemoveStunServer={state.handleRemoveStunServer}
          onResetStunDefaults={state.handleResetStunDefaults}
          onPrivateModeToggle={state.handlePrivateModeToggle}
          onConnectivityCheck={state.handleConnectivityCheck}
          onTorToggle={state.handleTorToggle}
          setStunServerInput={state.setStunServerInput}
        />
      );
    case "hub":
      return (
        <HubView
          identity={state.identity}
          toasts={state.toasts}
          removeToast={state.removeToast}
          generatedInvite={state.generatedInvite}
          inviteToConnect={state.inviteToConnect}
          inviteValid={state.inviteValid}
          namingMyName={state.namingMyName}
          namingTheirName={state.namingTheirName}
          isConnecting={state.isConnecting}
          onGenerateInvite={state.handleGenerateInvite}
          onCopyInvite={state.copyInvite}
          setInviteToConnect={state.setInviteToConnect}
          onConnect={state.handleConnect}
          setNamingMyName={state.setNamingMyName}
          setNamingTheirName={state.setNamingTheirName}
          onOpenChat={state.handleOpenChat}
          onOpenSettings={state.openSettings}
          onDeleteConversation={state.handleDeleteConversation}
          conversations={state.conversations}
          networkSettings={state.networkSettings}
          privateMode={state.privateMode}
        />
      );
    case "chat":
      return (
        <ChatView
          connection={state.connection}
          messages={state.messages}
          identity={state.identity}
          fileRequests={state.fileRequests}
          activeConversationId={state.activeConversationId}
          toasts={state.toasts}
          removeToast={state.removeToast}
          addToast={state.addToast}
          onSendMessage={state.handleSendMessage}
          onSendFile={state.handleSendFile}
          onVerify={state.handleVerify}
          onDisconnect={state.handleDisconnect}
          onBackToHub={() => state.setView("hub")}
          onExportConversation={state.handleExportConversation}
          onSetRetention={state.handleSetRetention}
          retentionPolicy={state.retentionPolicy}
          setRetentionPolicy={state.setRetentionPolicy}
          retentionDuration={state.retentionDuration}
          setRetentionDuration={state.setRetentionDuration}
        />
      );
    default:
      return <SetupView toasts={state.toasts} removeToast={state.removeToast} />;
  }
}

export default App;
