# M2M Design System

## Brand
M2M is a privacy-first, end-to-end encrypted messenger. The brand communicates trust, security, and modern minimalism. Think: Signal meets macOS, with a dark, premium aesthetic.

## Colors

### Dark Mode
- Background: `#030408` (deepest navy)
- Surface: `rgba(12, 14, 24, 0.82)` (glass card)
- Elevated: `rgba(28, 30, 44, 0.7)`
- Accent: `#6366f1` (indigo)
- Accent Bright: `#c7d2fe`
- Text Primary: `#f1f5f9`
- Text Secondary: `#cbd5e1`
- Text Muted: `#94a3b8`
- Success: `#10b981`
- Danger: `#ef4444`
- Warning: `#f59e0b`
- Border: `rgba(255, 255, 255, 0.08)`
- Input Background: `rgba(255, 255, 255, 0.05)`

### Light Mode
- Background: `#f0f2f5`
- Surface: `rgba(255, 255, 255, 0.85)`
- Accent: `#4f46e5`
- Text Primary: `#0f172a`

## Typography
- Body Font: `Inter`
- Mono Font: `JetBrains Mono`
- Weights: 400 (normal), 500 (medium), 600 (semibold), 700 (bold)
- Scale: 0.68rem (xs), 0.75rem (sm), 0.8125rem (base), 0.875rem (md), 0.9375rem (lg), 1.05rem (xl), 1.2rem (2xl), 1.4rem (3xl), 1.75rem (4xl)

## Spacing
4px base scale: 4, 8, 12, 16, 20, 24, 32, 40, 48, 64

## Border Radius
- Small: 8px
- Medium: 12px
- Large: 16px
- XL: 20px
- 2XL: 28px
- Full: 9999px

## Glassmorphism
- Backdrop blur: 24px (standard), 48px (heavy)
- Saturation: 180%
- Surface borders: `rgba(255, 255, 255, 0.04)`
- Edge light: subtle white gradient at top of cards

## Shadows
- Card: `0 2px 12px rgba(0, 0, 0, 0.2)`
- Card Hover: `0 8px 30px rgba(0, 0, 0, 0.35)`
- Accent Glow: `0 0 30px rgba(99, 102, 241, 0.12)`
- Modal: `0 25px 80px rgba(0, 0, 0, 0.7)`

## Key Screens

### 1. Hub (Conversation List + Connect)
- Glassmorphism floating card (1000px max width, centered)
- Tab bar: Connect, Chats, Nearby, Family
- Conversation items with gradient avatar, name, preview, timestamp
- Online/offline status dots with green glow
- Favorites with star indicator
- Search bar for conversations
- Empty states with icons and helpful text

### 2. Chat View (Message List + Input)
- Encrypted session banner with lock icon and glow animation
- Chat bubbles: sent (accent gradient, right-aligned), received (elevated surface, left-aligned)
- Message reactions bar (emoji picker on hover)
- Context menu (edit/delete) on right-click
- Self-destruct timer display
- Typing indicator with animated dots
- File transfer progress bars
- Text input with attach button, emoji picker, timer selector, send button
- Scroll-to-bottom FAB

### 3. Settings
- Section-based layout: Identity, Network, Discovery, Security, STUN, Theme, About
- Toggle switches for LAN, DHT, Private Mode, Tor, Screen Capture
- Select inputs for clipboard clear, idle lock timers
- Theme selector (light/dark/system)
- Accent color picker

### 4. Vault (Lock/Unlock)
- Centered card with lock/unlock icon
- Animated pulse ring on icon
- Passphrase input with show/hide toggle
- Strength meter bar
- Confirm passphrase for first-time setup
- Tips section for strong passphrases

### 5. Setup (Onboarding)
- 4-step wizard: Welcome → Identity → Encryption → Ready
- Step indicators with check marks
- Sonar ring animation on icon
- Crypto badge showing protocol names
