# NearbyTab — Implementation Prompt

## Mission

Implement the Nearby tab content inside HubView. This tab shows discovered peers on the local network (LAN) and DHT network, allowing the user to connect to discovered peers directly.

## Scope

Covers the Nearby tab including:
- Display of discovered LAN peers (📡) and DHT peers (🌐)
- Individual peer cards with name, address, key, and Connect button
- Discovery off state with privacy explanation
- No peers found state with Refresh button
- Privacy notice about ephemeral IDs

Does NOT cover: The HubView shell, actual discovery backend (LAN multicast, DHT), Settings toggles.

## Files Expected to Be Modified

- `src/views/NearbyTab.tsx` — Main component
- `src/styles/components/utilities.css` — Tab-specific styles
- `src/components/ui/icons/WifiIcon.tsx` — LAN indicator
- `src/components/ui/icons/GlobeIcon.tsx` — DHT indicator

## Components to Reuse

- **Card** (Section 2.3) — Individual peer display cards
- **Button** (Section 2.1) — Connect, Refresh, Open Settings actions
- **Badge** (Section 2.5) — Status indicators
- **OnlineDot** (Section 15.2) — Connection status

## Components to Create

- **PeerCard** — Single discovered peer with icon, name, address, key, Connect button
- **DiscoveryOffState** — Explanation + Open Settings action
- **NoPeersState** — Explanation + Refresh action
- **PrivacyNotice** — Yellow info box about ephemeral IDs

## Layout Hierarchy

```
<NearbyTab>
  <div class="nearby-tab">
    <!-- Discovery Off State -->
    <DiscoveryOffState>
      <WifiIcon size={48} muted />
      <h2>Discovery Not Active</h2>
      <p>Enable LAN or DHT discovery in Settings to find nearby peers.</p>
      <Button variant="default">Open Settings</Button>
    </DiscoveryOffState>

    <!-- No Peers Found -->
    <NoPeersState>
      <p>No LAN peers detected. Make sure other M2M users are on the same network.</p>
      <Button variant="secondary">Refresh</Button>
    </NoPeersState>

    <!-- Peer List -->
    <div class="nearby-list">
      <PeerCard
        type="lan"
        name="Unknown Peer"
        address="192.168.1.42:38553"
        key="a1b2c3d4..."
        time="2m ago"
        onConnect={handleConnect}
      />
      <PeerCard
        type="dht"
        name="DHT Peer"
        address="203.0.113.42:38553"
        key="f6e5d4c3..."
        time="5m ago"
        onConnect={handleConnect}
      />
    </div>

    <!-- Privacy Notice -->
    <PrivacyNotice>
      <p>⚠️ Both are OFF by default for privacy. When enabled, your IP address is visible to observers on the discovery channel. Ephemeral IDs are used (not your permanent identity key) and rotate periodically.</p>
    </PrivacyNotice>
  </div>
</NearbyTab>
```

## Design Implementation Requirements

### Exact Spacing

- Section to section gap: --space-2xl (32px)
- Card to card gap: --space-md (16px)
- Card internal padding: --space-lg (20px)
- Header icon to title gap: --space-sm (12px)
- Info row gap: 4px
- Connect button: right-aligned in card

### Typography

- Discovery off title: --text-lg, 600 weight
- Discovery off description: --text-md, --color-text-muted
- Peer name: --text-md, --font-weight-semibold
- Peer address: --text-sm, --font-mono, --color-text-muted
- Peer key: --text-xs, --font-mono, --color-text-muted
- Time label: --text-xs, --color-text-muted
- Privacy notice: --text-sm, --color-warning

### Colors

- LAN indicator accent: --color-accent
- DHT indicator accent: --color-success
- Connect button: default variant (accent)
- Privacy notice bg: --color-warning-bg
- Privacy notice text: --color-warning
- Privacy notice border: 1px --color-warning at 0.3 opacity

### Icons

- WifiIcon — LAN discovery type (16px)
- GlobeIcon — DHT discovery type (16px)
- ArrowRight / LinkIcon — Connect action

## States

| State | Visual | Behavior |
|-------|--------|----------|
| Discovery off | Explanation text, "Open Settings" button | Click opens SettingsView |
| No peers found | Explanation + "Refresh" button | Click refreshes peer scan |
| Peers found | List of PeerCards | Each has Connect button |
| Connecting | Button shows spinner | Waits for connection result |
| Connected | Button shows "Connected" badge | Disabled |

### Error States

| Trigger | Message | Type |
|---------|---------|------|
| D-004 | DHT toggle failed | toast, 5s |
| D-006 | Connect to discovered peer failed | toast, 6s |
| D-007 | Peer list refresh failed | toast, 4s |

## Interactions

- **Connect**: Click Connect on any peer → initiates X3DH handshake → navigate to ChatView on success
- **Refresh**: Click Refresh → rescans LAN/DHT for peers → updates list
- **Open Settings**: Click → navigate to SettingsView Discovery section
- **Auto-refresh**: Peer list refreshes every 30s while tab is active

## Accessibility

- Discovery status: role="status"
- Each peer card: role="listitem"
- Connect buttons: aria-label with peer name
- Privacy notice: role="alert"

## Security Considerations

- Ephemeral IDs used (not permanent identity key), rotate periodically
- Both discovery methods OFF by default (privacy-first)
- No permanent key exposed in discovery
- Privacy notice explains IP visibility clearly

## Acceptance Criteria

- [ ] Discovery off state shows explanation + "Open Settings"
- [ ] No peers state shows explanation + "Refresh"
- [ ] LAN peers shown with 📡 icon and IP:port
- [ ] DHT peers shown with 🌐 icon and IP:port
- [ ] Each peer has Connect button that shows spinner during connection
- [ ] Privacy notice visible when discovery is enabled
- [ ] Refresh button rescans for peers
- [ ] Error toasts on connection failure

## Self-Review Checklist

- [ ] Follows Design Bible Section 3.3c
- [ ] All states handled (off, empty, populated, error)
- [ ] Privacy-first defaults respected
- [ ] i18n strings match catalog
