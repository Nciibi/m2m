/**
 * Internationalisation catalog.
 *
 * ## Why this exists
 *
 * Every user-facing string was previously a hardcoded English literal in JSX.
 * For this app that is a security problem, not just a polish problem: the two
 * most consequential prompts in the product — arming the panic wipe and
 * registering a duress passphrase — were `window.confirm` / `window.prompt`
 * calls. Native dialogs cannot be styled, cannot be localised, and are a
 * well-known target for UI spoofing. Someone under coercion needs to read
 * "this will irreversibly delete everything" in a language they actually
 * read; a generic `OK / Cancel` in a locale they can't parse is not a
 * meaningful confirmation.
 *
 * ## Shape
 *
 * `en` is the source of truth and is fully populated. Other locales are
 * `Partial<Translation>`, so a missing key falls back to English rather than
 * rendering a raw key at the user. `t()` is typed against the `en` shape, so
 * adding a key to `en` and forgetting to translate it is a visible diff rather
 * than a runtime surprise.
 *
 * ## Adding a locale
 *
 * 1. Copy the `en` block.
 * 2. Translate the values (keys stay in English).
 * 3. Register it in `LOCALES` below.
 * 4. Add an entry to `src/types.ts` `Locale` union.
 *
 * ## Interpolating values
 *
 * Use `{{name}}` placeholders. Values are inserted by React as children, so a
 * peer-controlled string can never become markup.
 */

export const en = {
  // ─── Generic ────────────────────────────────────────────────────────────
  generic: {
    cancel: "Cancel",
    confirm: "Confirm",
    close: "Close",
    back: "Back",
    copy: "Copy",
    reset: "Reset",
    add: "Add",
    remove: "Remove",
    refresh: "Refresh",
    hub: "Hub",
    save: "Save",
    loading: "Loading…",
    unknown: "unknown",
    backToUnlock: "Back to unlock",
    createAnotherAccount: "Create another account",
    startMessaging: "Start Messaging",
    getStarted: "Get Started",
    next: "Next",
  },

  // ─── Navigation / chrome ────────────────────────────────────────────────
  nav: {
    chats: "Chats",
    groups: "Groups",
    settings: "Settings",
    secure: "Secure",
    mainNavigation: "Main navigation",
    sections: "Sections",
    lockVault: "Lock Vault",
    tabConnect: "Connect",
    tabChats: "Chats",
    tabNearby: "Nearby",
    tabFamily: "Family",
  },

  // ─── Onboarding ─────────────────────────────────────────────────────────
  setup: {
    title: "Welcome to M2M",
    subtitle: "A private, end-to-end encrypted messenger. No servers, no accounts, no tracking.",
    localTitle: "Your Identity is Local",
    localBody: "Your keys are generated on this device and never leave it.",
    e2eeTitle: "End-to-End Encrypted",
    e2eeBody:
      "Messages use X3DH + Double Ratchet (Signal protocol). Ed25519 signing, X25519 key exchange, XChaCha20-Poly1305 encryption.",
    readyTitle: "Ready to Go!",
    readyBody:
      "Share your invite link with a trusted peer to start chatting. Both sides must generate and share invites.",
    initialising: "Initializing Secure Enclave",
    generatingKeys: "Generating Ed25519 identity keys. They never leave your device.",
    generatingKeysLabel: "Generating identity keys",
    stepsLabel: "Onboarding steps",
    crypto: "Ed25519 · X25519 · XChaCha20-Poly1305",
    finalizeFailed: "Failed to finalize setup",
  },

  // ─── Vault ──────────────────────────────────────────────────────────────
  vault: {
    createAccount: "Create Account",
    createVault: "Create Vault",
    unlock: "Unlock",
    unlockTitle: "Unlock Your Vault",
    setupTitle: "Set Up Your Vault",
    createAnotherTitle: "Create Another Account",
    unlockDesc: "Enter your passphrase to decrypt your local data.",
    setupDesc:
      "Choose a strong passphrase to encrypt your identity keys and message history.",
    createDesc:
      "Choose a strong passphrase for the new account — it selects this account on unlock.",
    passphrase: "Passphrase",
    passphrasePlaceholder: "Enter your passphrase",
    confirmPassphrase: "Confirm",
    confirmPlaceholder: "Repeat your passphrase",
    minLength: "Passphrase must be at least 12 characters.",
    mismatch: "Passphrases do not match.",
    tooWeak: "Passphrase too weak: ~{{bits}} bits. Use longer (aim for 60+).",
    tooShortLabel: "Too short (min 12)",
    kdfHint: "Minimum 12 chars · Argon2id",
    belongsTo: "This vault belongs to {{fingerprint}}…",
    createFailed: "Account creation failed.",
    unlockFailed: "Unlock failed. Check your passphrase.",
    onScreenKeyboard: "On-screen keyboard (bypasses hardware keyloggers)",
    onScreenKeyboardLabel: "Toggle on-screen keyboard",
    onScreenKeyboardHint: "Tap keys — layout reshuffles after each press",
    onScreenKeyboardClose: "Close on-screen keyboard",
    toggleUppercase: "Toggle uppercase",
    backspace: "Backspace",
    paste: "Paste",
    pasteLabel: "Paste passphrase",
    hidePassphrase: "Hide passphrase",
    showPassphrase: "Show passphrase",
    strengthLabel: "{{label}} — {{bits}} bits",
    charCount: "{{count}} chars",
    toggleTips: "Hide tips",
    showTips: "What makes a strong passphrase?",
    tipsTitle: "Tips:",
    tip1: "Use 5+ random words (diceware method)",
    tip2: "Aim for 60+ bits of entropy",
    tip3: "Avoid common phrases or song lyrics",
    tip4: "Include a mix of cases, numbers, or symbols",
    tip5: '"correct-horse-battery-staple" style is excellent',
  },

  // ─── Chat ───────────────────────────────────────────────────────────────
  chat: {
    verified: "Verified",
    verify: "Verify",
    encryptedSession: "Encrypted Session",
    e2eBanner: "End-to-end encrypted session established.",
    footerEncrypted: "End-to-end encrypted",
    footerHint: "Ctrl+Enter to send · Esc to go back",
    composerPlaceholder: "Type a secure message…",
    send: "Send",
    sendFile: "Send file",
    emojiPicker: "Emoji picker",
    dropFiles: "Drop files here to send",
    droppedToDialog: "Dropped {{name}} — sending via file dialog…",
    timer: "Self-destruct timer",
    timerTitle: "Self-destructs in {{mins}}m {{secs}}s",
    selfDestructsIn: "🔥 {{mins}}:{{secs}}",
    retry: "Reconnect",
    reconnecting: "Reconnecting…",
    reconnectingCount: "Reconnecting ({{attempt}}/5)…",
    disconnect: "Disconnect",
    scrollToBottom: "Scroll to bottom",
    closeSearch: "Close search",
    searchPlaceholder: "Search messages… (Esc to close)",
    searchResultCount: "{{count}} result",
    searchResultCountPlural: "{{count}} results",
    searchFailed: "Search failed",
    transcriptLabel: "Message transcript",
    loadingOlder: "Loading older messages…",
    beginningOfConversation: "Beginning of conversation",
    peerTyping: "Peer is typing…",
    someoneTyping: "Someone is typing…",
    newMessageFrom: "New message from {{who}}",
    newMessageFromYou: "New message from you",
    emptyTitle: "Start the conversation",
    emptyDesc:
      "Send a message below to begin your encrypted conversation. All messages are protected with end-to-end encryption.",
    conversationPolicy: "Conversation Policy",
    noExpiration: "No Expiration",
    autoDeleteAfter: "Auto-Delete After",
    autoExportAfter: "Auto-Export After",
    oneHour: "1 Hour",
    twentyFourHours: "24 Hours",
    sevenDays: "7 Days",
    exportNow: "Export Now",
    selectFileToSend: "Select file to send",
    saveIncomingFile: "Save incoming file",
    exportConversation: "Export Conversation",
    accept: "Accept",
    reject: "Reject",
    acceptFailed: "Accept failed: {{err}}",
    rejectFailed: "Reject failed: {{err}}",
    saveAsTitle: 'Save "{{filename}}"',
    secondsRemaining: "· {{secs}}s remaining",
    messageDeleted: "Message deleted",
    deletedPlaceholder: "[deleted]",
    // Fingerprint comparison
    verifyTitle: "Verify Peer Fingerprint",
    verifyDesc: "Compare fingerprints via a secure out-of-band channel.",
    youLocal: "You (Local)",
    matched: "Matched",
    notYetVerified: "Not yet verified",
    peer: "Peer",
    peerVerifiedBanner: "Peer verified",
    confirmMatchAndVerify: "Confirm Match & Verify",
    verificationFailed: "Verification failed: {{err}}",
    peerVerified: "Peer verified",
    // Bubble
    senderYou: "you",
    senderPeer: "peer",
    messageFrom: "Message from {{sender}}",
    toggleReactionPicker: "Toggle reaction picker",
    messageOptions: "Message options",
    editAction: "Edit",
    deleteAction: "Delete",
    reactAction: "React {{emoji}}",
    editedAt: "Edited {{when}}",
    readAt: "Read {{when}}",
    timerOff: "Off",
  },

  // ─── Hub ────────────────────────────────────────────────────────────────
  hub: {
    connecting: "Connecting…",
    connected: "Connected",
    offline: "Offline",
    appName: "M2M",
    settingsLabel: "Settings",
    hostTitle: "Host a Connection",
    hostDesc:
      "Generate a one-time signed invite for a peer to connect to you securely.",
    listening: "Listening for incoming connections",
    generateInvite: "Generate Invite Link",
    copyInviteLabel: "Copy invite",
    expiresIn: "Expires in {{mins}}m:{{secs}}",
    recentInvites: "Recent Invites",
    torWarningTitle: "Tor Inbound Warning",
    torWarningBody:
      "Tor is enabled for outbound connections, but this invite contains your real IP address.",
    joinTitle: "Join a Connection",
    joinDesc: "Paste an invite link from a trusted peer to connect.",
    invitePlaceholder: "m2m://...",
    connect: "Connect",
    validInvite: "Valid Invite Found",
    yourName: "Your Name",
    yourNamePlaceholder: "How they will see you",
    theirName: "Their Name",
    theirNamePlaceholder: "How you want to see them",
    yourFingerprint: "Your Identity Fingerprint",
    copyLabel: "Copy",
    searchConversations: "Search conversations…",
    noConversationsFound: "No conversations found",
    noConversationsYet: "No conversations yet",
    noConversationsSearchDesc: "Try adjusting your search terms or clear the filter.",
    noConversationsDesc:
      "Generate an invite link to host a connection, or paste an invite from a peer to join.",
    unknownPeer: "Unknown Peer",
    noMessagesYet: "No messages yet.",
    unfavorite: "Unfavorite",
    favorite: "Favorite",
    unarchive: "Unarchive",
    archive: "Archive",
    unmute: "Unmute",
    mute: "Mute",
    unmuteLabel: "Unmute conversation",
    muteLabel: "Mute conversation",
    deleteLabel: "Delete",
    discoveryNotActive: "Discovery Not Active",
    discoveryNotActiveDesc:
      "Enable LAN or DHT discovery in Settings to find nearby peers. Both are OFF by default — privacy first.",
    openSettings: "Open Settings",
    noPeersFound: "No Peers Found",
    noLanPeers:
      "No LAN peers detected. Make sure other M2M users are on the same network with LAN discovery enabled.",
    noDhtPeers:
      "No DHT peers found. They may be offline or behind a symmetric NAT.",
    lanPeer: "LAN Peer",
    dhtPeer: "DHT Peer",
    badgeLan: "LAN",
    badgeDht: "DHT",
  },

  // ─── Groups ─────────────────────────────────────────────────────────────
  group: {
    title: "Group Chats",
    newGroup: "New Group",
    groupName: "Group Name",
    groupNamePlaceholder: "My Group",
    memberKeys: "Member Peer Keys (comma-separated hex)",
    memberKeysPlaceholder: "aabbccdd…, eeff0011…",
    createGroup: "Create Group",
    needMembers: "Add at least one member (64-char hex key)",
    created: "Group created!",
    createFailed: "Failed to create group: {{err}}",
    sendFailed: "Failed to send: {{err}}",
    members: "{{count}} members",
    tapToOpen: "Tap to open group chat",
    noGroupsYet: "No groups yet",
    noGroupsDesc: "Create a group to start an encrypted group conversation.",
    noMessagesYet: "No messages yet",
    startConversation: "Start the conversation!",
    composerPlaceholder: "Type a group message…",
    footer: "Group E2EE · Sender Keys",
    footerHint: "Enter to send",
  },

  // ─── Family ─────────────────────────────────────────────────────────────
  family: {
    noMembers: "No family members",
    noMembersDesc:
      "Add people you trust to message them without generating an invite each time.",
    addToFamily: "Add to Family",
    add: "Add",
    memberCount: "{{count}} member",
    memberCountPlural: "{{count}} members",
    expired: "Expired",
    daysLeft: "{{days}}d left",
    forever: "Forever",
    lastAddress: "· {{addr}}",
    renew: "Renew",
    update: "Update",
    newInvitePlaceholder: "Paste new invite…",
    msg: "Msg",
    removeFailed: "Failed to remove: {{err}}",
    updated: "Family member updated",
    updateFailed: "Update failed: {{err}}",
    requiredFields: "Peer key and nickname are required",
    added: "Added to family",
    addFailed: "Failed to add: {{err}}",
    modalTitle: "Add to Family",
    peerKey: "Peer Key",
    peerKeyPlaceholder: "Peer public key hex",
    nickname: "Nickname",
    nicknamePlaceholder: "How you'll know them",
    duration: "Duration",
    sevenDays: "7 days",
    thirtyDays: "30 days",
    ninetyDays: "90 days",
    custom: "Custom",
    days: "Days",
  },

  // ─── Settings ───────────────────────────────────────────────────────────
  settings: {
    title: "Settings",
    identity: "Identity",
    fingerprint: "Fingerprint",
    emDash: "—",
    copyFingerprintLabel: "Copy fingerprint",
    publicKey: "Public Key",
    copyIpLabel: "Copy IP",
    signOut: "Sign Out",
    signOutHint: "Locks the vault and returns to the unlock screen.",
    signOutFailed: "Failed to sign out: {{err}}",
    network: "Network",
    publicIp: "Public IP",
    ipNotDiscovered: "Not yet discovered",
    discoverViaStun: "Discover via STUN",
    natType: "NAT Type",
    stunServers: "STUN Servers",
    reachableCount: "{{count}} reachable",
    privateMode: "Private Mode",
    privateModeLabel: "Toggle private mode",
    privateModeHint: "Hide IP from invites",
    tor: "Tor",
    torLabel: "Toggle Tor",
    torHint: "Route connections via Tor",
    testTor: "Test Tor",
    testingTor: "Testing Tor…",
    torReachable: "Tor ✓",
    torUnreachable: "Tor not reachable via current proxy",
    torTestUnavailable: "Tor test unavailable: {{err}}",
    connectivity: "Connectivity",
    check: "Check",
    result: "Result",
    discovery: "Discovery",
    lanDiscovery: "LAN Discovery",
    lanLabel: "Toggle LAN discovery",
    lanHint: "Broadcast presence on local WiFi",
    dhtDiscovery: "DHT Discovery",
    dhtLabel: "Toggle DHT discovery",
    dhtHint: "Discover peers via DHT network",
    discoveredPeers: "Discovered Peers",
    foundCount: "{{count}} found",
    discoveryWarning:
      "Both are OFF by default for privacy. When enabled, your IP address is visible to observers on the discovery channel. Ephemeral IDs are used (not your permanent identity key) and rotate periodically.",
    security: "Security",
    screenCapture: "Screen Capture Protection",
    screenCaptureLabel: "Toggle screen capture protection",
    screenCaptureHint: "Prevent window from appearing in screenshots",
    captureFull: "Full protection on this platform",
    capturePartial: "Partial protection on this platform",
    captureNone: "Not effective on this platform",
    captureDetection: "Capture Software Detection",
    captureDetectionLabel: "Toggle capture software detection",
    captureDetectionHint:
      "Warn while OBS, Snipping Tool, and other recorders are running (stops nothing — detection only)",
    blurOnUnfocus: "Blur When Unfocused",
    blurLabel: "Toggle blur when window loses focus",
    blurHint: "Blur all content whenever the window loses focus or is hidden",
    airGap: "Air-Gap Mode",
    airGapLabel: "Toggle air-gap mode",
    airGapHint:
      "LAN-only: blocks STUN, port forwarding, relay registration, discovery, and Tor",
    ephemeral: "Ephemeral Conversations",
    ephemeralLabel: "Toggle ephemeral conversations",
    ephemeralHint: "RAM only: no message, reaction, or edit is ever written to disk",
    sendBatching: "Send Batching Delay",
    sendBatchingLabel: "Random send delay for traffic analysis resistance",
    batchingOff: "Off",
    batchingLow: "~0–250ms",
    batchingMed: "~0–1s",
    batchingHigh: "~0–5s",
    sendBatchingHint: "Random pre-send delay so message timing leaks less",
    typingCover: "Typing Cover Traffic",
    typingCoverLabel: "Toggle typing indicator cover traffic",
    typingCoverHint: "Randomize typing-indicator timing so keystroke cadence leaks less",
    duress: "Duress Passphrase",
    duressSet: "Set…",
    duressRegisteredHint: "Registered — entering it at unlock wipes the vault",
    duressHint: "Coercion resistance: a special passphrase that wipes everything",
    panicHotkey: "Panic Hotkey (Ctrl+Alt+Shift+W)",
    panicHotkeyLabel: "Arm emergency panic wipe hotkey",
    panicArmedHint: "ARMED — hotkey deletes everything instantly, no confirmation",
    panicHint: "Emergency wipe: delete all data and exit instantly",
    clipboardAutoClear: "Clipboard Auto-Clear",
    clipboardAutoClearLabel: "Clipboard auto-clear timeout",
    clipboardHint: "Auto-clear clipboard after copying sensitive data",
    idleLock: "Idle Vault Lock",
    idleLockLabel: "Idle vault lock timeout",
    idleLockHint: "Auto-lock vault after inactivity",
    knownContacts: "Known Contacts Only",
    knownContactsLabel: "Toggle known contacts only",
    knownContactsHint:
      "Reject incoming connections from strangers (first-time invites require turning this off)",
    vaultSection: "Vault",
    lockNow: "Lock Now",
    clearClipboard: "Clear Clipboard",
    theme: "Theme",
    appearance: "Appearance",
    lightTheme: "Light theme",
    light: "Light",
    darkTheme: "Dark theme",
    dark: "Dark",
    systemTheme: "System theme",
    system: "System",
    currentTheme: "Current: {{theme}}",
    accentColor: "Accent Color",
    accentLabel: "Accent color",
    resetAccentLabel: "Reset accent color",
    about: "About",
    version: "Version",
    crypto: "Ed25519 · X25519 · XChaCha20-Poly1305 · X3DH · Double Ratchet",
    ok: "OK",
    fail: "FAIL",
    addServerPlaceholder: "host:port",
    resetStunLabel: "Reset STUN servers to defaults",

    // ── Destructive-action dialogs ──
    // These were `window.confirm` / `window.prompt`. Native dialogs cannot be
    // styled, cannot be localised, and are a spoofing target — unacceptable for
    // irreversible, life-safety decisions.
    duressSetTitle: "Set duress passphrase",
    duressSetBody:
      "A duress passphrase is a second, distinct passphrase. Entering it at the unlock screen silently deletes all local data and then shows a normal wrong-passphrase error — there is no confirmation at unlock, because that is the entire point.",
    duressSetWarning: "This cannot be undone.",
    duressSetLabel: "Duress passphrase",
    duressSetPlaceholder: "At least 12 characters, different from your main passphrase",
    duressConfirmTitle: "Register duress passphrase?",
    duressConfirmBody:
      "Entering this passphrase at unlock will irreversibly delete all local data. You will not be warned, and there is no recovery.",
    duressConfirmAction: "Wipe on this passphrase",
    panicArmTitle: "Arm panic hotkey?",
    panicArmBody:
      "Ctrl+Alt+Shift+W will IMMEDIATELY delete all local data and close M2M. No confirmation. No undo.",
    panicArmAction: "Arm it",
  },

  // ─── Toasts / errors ────────────────────────────────────────────────────
  toast: {
    securityLoadFailed: "Could not load security settings — protections may be inactive",
    autoLockFailed: "Auto-lock FAILED — lock the vault manually",
    clipboardAutoClearFailed: "Clipboard auto-clear FAILED — clear it manually",
    clipboardCleared: "Clipboard cleared",
    clipboardClearFailed: "Failed to clear clipboard: {{err}}",
    panicArmed: "Panic hotkey ARMED — Ctrl+Alt+Shift+W wipes everything",
    panicDisarmed: "Panic hotkey disarmed",
    panicToggleFailed: "Failed to toggle panic hotkey: {{err}}",
    duressRegistered:
      "Duress passphrase registered — entering it at unlock will WIPE the vault",
    duressRemoved: "Duress passphrase removed",
    duressRegisterFailed: "Failed to register duress passphrase: {{err}}",
    duressRemoveFailed: "Failed to remove duress passphrase: {{err}}",
    vaultLocked: "Vault locked",
    vaultLockFailed: "Failed to lock vault: {{err}}",
    knownContactsOn: "Known contacts only — strangers can no longer connect",
    knownContactsOff: "Known contacts only disabled — anyone may connect",
    knownContactsFailed: "Failed to toggle known contacts only: {{err}}",
    airGapOn: "Air-gap mode ON — internet-facing operations blocked (LAN only)",
    airGapOff: "Air-gap mode off — internet operations allowed again",
    airGapFailed: "Failed to toggle air-gap mode: {{err}}",
    ephemeralOn: "Ephemeral mode ON — conversations stay in RAM only",
    ephemeralOff: "Ephemeral mode off — conversations persist to encrypted storage",
    ephemeralFailed: "Failed to toggle ephemeral mode: {{err}}",
    captureOn: "Screen capture protection enabled",
    captureOff: "Screen capture protection disabled",
    captureFailed: "Failed to toggle screen capture protection: {{err}}",
    captureDetectOn: "Capture software detection enabled",
    captureDetectOff: "Capture software detection disabled",
    captureDetectFailed: "Failed to toggle capture detection: {{err}}",
    blurFailed: "Failed to toggle focus blur: {{err}}",
    clipboardSettingFailed: "Failed to update clipboard setting: {{err}}",
    idleLockFailed: "Failed to update idle lock setting: {{err}}",
    batchingFailed: "Failed to update send batching: {{err}}",
    typingCoverFailed: "Failed to toggle typing cover traffic: {{err}}",
    privateModeFailed: "Failed to {{verb}} private mode: {{err}}",
    privateModeEnable: "enable",
    privateModeDisable: "disable",
    stunFailed: "STUN failed: {{err}}",
    stunAddFailed: "Failed to add STUN server: {{err}}",
    stunRemoveFailed: "Failed to remove STUN server: {{err}}",
    stunResetFailed: "Failed to reset STUN servers: {{err}}",
    stunRemoveLast: "Cannot remove all STUN servers — at least one required.",
    connectivityFailed: "Connectivity check failed: {{err}}",
    torFailed: "Tor toggle failed: {{err}}",
    lanFailed: "LAN discovery toggle failed: {{err}}",
    dhtFailed: "DHT discovery toggle failed: {{err}}",
    discoveredConnected: "Connected to discovered peer",
    discoveredConnectFailed: "Connection to discovered peer failed: {{err}}",
    discoveryRefreshFailed: "Refresh discovery failed: {{err}}",
    themeFailed: "Failed to save theme: {{err}}",
    fileRequestSent: "File request sent: {{filename}}",
    sendFileFailed: "Failed to send file: {{err}}",
    downloadingFile: "Downloading file...",
    acceptTransferFailed: "Failed to accept transfer: {{err}}",
    transferRejected: "File transfer rejected",
    rejectTransferFailed: "Failed to reject transfer: {{err}}",
    exported: "Exported successfully",
    exportFailed: "Export failed: {{err}}",
    connectionFailed: "Connection failed: {{err}}",
    editFailed: "Edit failed: {{err}}",
    deleteFailed: "Delete failed: {{err}}",
    openConversationFailed: "Could not open conversation: {{err}}",
    transferComplete: "File transfer complete: {{filename}}",
    transferFailed: "File transfer failed: {{err}}",
    transferFailedUnknown: "unknown error",
    transferCancelled: "File transfer cancelled",
    notConnected: "Not connected",
    noActivePeer: "No active peer to verify",
    notificationTitle: "M2M",
    notificationBody: "New message from {{who}}",
  },

  // ─── Security / capture warnings ─────────────────────────────────────────
  security: {
    captureDetected:
      "⚠ Screen capture software detected: {{apps}} — your screen may be recorded.",
  },

  // ─── Errors / boundaries / shortcuts ────────────────────────────────────
  error: {
    viewCrashed: "{{name}} Crashed",
    unexpected: "An unexpected error occurred.",
    reload: "Reload",
    panicWipeRefused: "Panic wipe refused: {{err}}",
  },
  shortcuts: {
    title: "Keyboard Shortcuts",
    esc: "Esc",
    escDesc: "Go back to hub (from chat)",
    ctrlComma: "Ctrl+,",
    ctrlCommaDesc: "Open settings",
    ctrlEnter: "Ctrl+Enter",
    ctrlEnterDesc: "Send message",
    question: "?",
    questionDesc: "Toggle this help modal",
  },
  a11y: {
    closeDialog: "Close dialog",
    dismissNotification: "Dismiss notification",
    clearInput: "Clear input",
    progress: "Progress",
    reactPicker: "Toggle reaction picker",
  },

  // ─── Relative time ───────────────────────────────────────────────────────
  time: {
    now: "now",
    minutesAgo: "{{count}}m ago",
    hoursAgo: "{{count}}h ago",
    daysAgo: "{{count}}d ago",
    today: "Today",
    yesterday: "Yesterday",
  },
} as const;

/** Shape of a complete translation. Derived from `en`, so it can never drift. */
export type Translation = {
  [K in keyof typeof en]: {
    [P in keyof (typeof en)[K]]: string;
  };
};

/** A translation may be partial; missing keys fall back to English. */
export type PartialTranslation = {
  [K in keyof typeof en]?: Partial<(typeof en)[K]>;
};

export type LocaleCode = "en";

export const LOCALES: Record<LocaleCode, PartialTranslation> = {
  en,
};

/** Interpolate `{{name}}` placeholders. React renders the result as text. */
export function interpolate(
  template: string,
  values?: Record<string, string | number>,
): string {
  if (!values) return template;
  return template.replace(/\{\{(\w+)\}\}/g, (match, key: string) =>
    key in values ? String(values[key]) : match,
  );
}

/**
 * Look up a translation.
 *
 * The path is a dotted key, e.g. `t("chat.verified")`. Typing is loose here on
 * purpose — a strict dot-path type would be pleasant but adds a lot of
 * ceremony for a catalog this size. Unknown keys return the key itself, which
 * is visible in the UI rather than silently rendering an empty string.
 */
export function translate(locale: LocaleCode, path: string): string {
  const bundle = LOCALES[locale] ?? en;
  const parts = path.split(".");
  let node: unknown = bundle;
  for (const part of parts) {
    if (node && typeof node === "object" && part in (node as object)) {
      node = (node as Record<string, unknown>)[part];
    } else {
      // Fall back to English before giving up.
      node = parts.reduce<unknown>((acc, p) => {
        if (acc && typeof acc === "object" && p in (acc as object)) {
          return (acc as Record<string, unknown>)[p];
        }
        return undefined;
      }, en);
      if (node === undefined) return path;
      break;
    }
  }
  return typeof node === "string" ? node : path;
}

/**
 * Build a translator bound to one locale.
 *
 * ```ts
 * const t = useT();
 * t("chat.verified");
 * t("toast.exportFailed", { err: "disk full" });   // "Export failed: disk full"
 * ```
 */
export function makeT(locale: LocaleCode) {
  return (path: string, values?: Record<string, string | number>): string =>
    interpolate(translate(locale, path), values);
}
