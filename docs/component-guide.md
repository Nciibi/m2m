# M2M Component Usage Guide

Comprehensive documentation for all UI components in the M2M design system.

---

## Table of Contents

1. [Button](#button)
2. [Input](#input)
3. [Card](#card)
4. [Badge](#badge)
5. [Modal](#modal)
6. [Select](#select)
7. [Toast](#toast)
8. [LoadingSpinner](#loadingspinner)
9. [Icons](#icons)

---

## Button

Versatile button component with multiple variants, sizes, and states.

### Import

```tsx
import Button from '@/components/ui/Button';
```

### Props

```tsx
interface ButtonProps {
  variant?: "default" | "secondary" | "danger" | "ghost" | "icon";
  loading?: boolean;
  icon?: ReactNode;
  fullWidth?: boolean;
  size?: "sm" | "xs";
  children?: ReactNode;
  disabled?: boolean;
  className?: string;
  // + all native button HTML attributes
}
```

### Variants

#### Default (Primary)
High-emphasis button for primary actions. Features gradient background and shine effect.

```tsx
<Button variant="default">
  Create Connection
</Button>
```

**Visual:** Indigo gradient with accent shadow, hover lift animation.

**Use cases:**
- Primary CTAs (Create, Connect, Send)
- Form submissions
- Confirmation actions

---

#### Secondary
Medium-emphasis button for secondary actions.

```tsx
<Button variant="secondary">
  Cancel
</Button>
```

**Visual:** Subtle background, muted text color, no shadow.

**Use cases:**
- Cancel actions
- Secondary options
- Non-critical actions

---

#### Danger
Destructive actions requiring caution.

```tsx
<Button variant="danger">
  Delete Conversation
</Button>
```

**Visual:** Red text and border, transparent background.

**Use cases:**
- Delete operations
- Destructive confirmations
- Critical warnings

---

#### Ghost
Low-emphasis button for tertiary actions.

```tsx
<Button variant="ghost">
  Learn More
</Button>
```

**Visual:** Transparent background, subtle hover state.

**Use cases:**
- Tertiary actions
- In-content links
- Dismissible actions

---

#### Icon
Square button optimized for icon-only actions.

```tsx
<Button variant="icon" aria-label="Close">
  <CloseIcon size={20} />
</Button>
```

**Visual:** Square shape (42x42px), border, centered icon.

**Use cases:**
- Icon-only actions
- Toolbars
- Compact interfaces

**Accessibility:** Always provide `aria-label` for icon-only buttons.

---

### Sizes

```tsx
{/* Large (default) */}
<Button>Large Button</Button>

{/* Small */}
<Button size="sm">Small Button</Button>

{/* Extra Small */}
<Button size="xs">XS Button</Button>
```

**Dimensions:**
- Large: 16px vertical padding, 20px horizontal padding
- Small: 10px vertical padding, 16px horizontal padding
- Extra Small: 6px vertical padding, 12px horizontal padding

---

### States

#### Loading
Shows spinner and disables interaction.

```tsx
<Button loading>
  Processing...
</Button>
```

**Behavior:**
- Replaces content with spinner
- Automatically disables button
- Maintains button width (prevents layout shift)

---

#### Disabled
Prevents interaction and reduces opacity.

```tsx
<Button disabled>
  Unavailable
</Button>
```

**Visual:** 45% opacity, no hover effects, `not-allowed` cursor.

---

### With Icons

```tsx
<Button icon={<SendIcon size={18} />}>
  Send Message
</Button>
```

**Layout:** Icon positioned to the left of text with 8px gap.

---

### Full Width

```tsx
<Button fullWidth>
  Continue
</Button>
```

Stretches button to 100% of parent container width.

---

### Examples

```tsx
// Primary action with icon
<Button icon={<PlusIcon size={18} />}>
  New Connection
</Button>

// Loading state
const [loading, setLoading] = useState(false);
<Button loading={loading} onClick={handleSubmit}>
  Submit
</Button>

// Danger action with confirmation
<Button 
  variant="danger" 
  onClick={() => setConfirmModal(true)}
>
  Delete Account
</Button>

// Icon-only toolbar button
<Button variant="icon" aria-label="Settings">
  <GearIcon size={20} />
</Button>
```

---

## Input

Flexible text input with icons, error states, and clear functionality.

### Import

```tsx
import Input from '@/components/ui/Input';
```

### Props

```tsx
interface InputProps {
  icon?: ReactNode;
  error?: string;
  clearable?: boolean;
  onClear?: () => void;
  compact?: boolean;
  mono?: boolean;
  value?: string;
  onChange?: (e: React.ChangeEvent<HTMLInputElement>) => void;
  className?: string;
  // + all native input HTML attributes
}
```

### Basic Usage

```tsx
<Input 
  placeholder="Enter your name"
  value={name}
  onChange={(e) => setName(e.target.value)}
/>
```

---

### With Icon

```tsx
<Input 
  icon={<SearchIcon size={18} />}
  placeholder="Search conversations..."
  value={query}
  onChange={(e) => setQuery(e.target.value)}
/>
```

**Layout:** Icon positioned at left edge with 12px padding.

---

### Clearable

Shows clear button (×) when input has value.

```tsx
<Input 
  clearable
  value={text}
  onChange={(e) => setText(e.target.value)}
  onClear={() => setText('')}
/>
```

**Behavior:**
- Clear button appears only when value is not empty
- Clicking clear button triggers `onClear` and refocuses input
- 16px close icon with hover state

---

### Error State

```tsx
<Input 
  placeholder="Passphrase"
  type="password"
  value={passphrase}
  onChange={(e) => setPassphrase(e.target.value)}
  error="Passphrase must be at least 8 characters"
/>
```

**Visual:**
- Red border color
- Error message below input (red text, small size)
- Error state removed on focus

---

### Compact Mode

Reduces vertical padding for tight layouts.

```tsx
<Input 
  compact
  placeholder="Port number"
  value={port}
  onChange={(e) => setPort(e.target.value)}
/>
```

**Use cases:**
- Dense forms
- Inline editing
- Settings panels

---

### Monospace Font

Uses monospace font for technical input.

```tsx
<Input 
  mono
  placeholder="Public key fingerprint"
  value={fingerprint}
  readOnly
/>
```

**Use cases:**
- Keys and fingerprints
- Code snippets
- Technical identifiers
- Numeric codes

---

### Examples

```tsx
// Search with icon and clear
<Input 
  icon={<SearchIcon size={18} />}
  clearable
  placeholder="Search..."
  value={search}
  onChange={(e) => setSearch(e.target.value)}
  onClear={() => setSearch('')}
/>

// Password with toggle visibility
const [show, setShow] = useState(false);
<Input 
  type={show ? 'text' : 'password'}
  icon={<LockIcon size={18} />}
  placeholder="Passphrase"
  value={pass}
  onChange={(e) => setPass(e.target.value)}
/>

// Validated input with error
<Input 
  placeholder="Email address"
  value={email}
  onChange={(e) => setEmail(e.target.value)}
  error={!isValidEmail(email) ? 'Invalid email format' : ''}
/>
```

---

## Card

Container component for grouped content with optional header.

### Import

```tsx
import Card from '@/components/ui/Card';
```

### Props

```tsx
interface CardProps {
  children: ReactNode;
  header?: { 
    icon: ReactNode; 
    title: string; 
    iconVariant?: "accent" | "success" | "warning" | "danger" 
  };
  description?: string;
  clickable?: boolean;
  onClick?: () => void;
  style?: CSSProperties;
  className?: string;
  id?: string;
}
```

### Basic Usage

```tsx
<Card>
  <p>Card content goes here</p>
</Card>
```

---

### With Header

```tsx
<Card 
  header={{ 
    icon: <ShieldIcon size={20} />, 
    title: "Security Settings",
    iconVariant: "accent"
  }}
>
  <p>Configure your security preferences</p>
</Card>
```

**Icon Variants:**
- `accent`: Indigo background (default)
- `success`: Green background
- `warning`: Amber background
- `danger`: Red background

---

### With Description

```tsx
<Card 
  header={{ 
    icon: <KeyIcon size={20} />, 
    title: "Identity" 
  }}
  description="Your public key fingerprint and identity information"
>
  <div className="key-display">...</div>
</Card>
```

---

### Clickable Card

Makes entire card interactive.

```tsx
<Card 
  clickable
  onClick={() => navigate('/settings')}
  header={{ 
    icon: <GearIcon size={20} />, 
    title: "Settings" 
  }}
>
  <p>Configure application preferences</p>
</Card>
```

**Accessibility:**
- `role="button"` applied
- `tabIndex={0}` for keyboard navigation
- Enter and Space key support
- Hover and focus states

---

### Examples

```tsx
// Connection invitation card
<Card 
  header={{ 
    icon: <LinkIcon size={20} />, 
    title: "Generate Invite",
    iconVariant: "accent"
  }}
  description="Create a secure connection link to share"
>
  <Button fullWidth onClick={generateInvite}>
    Create Invite Link
  </Button>
</Card>

// Status card with success variant
<Card 
  header={{ 
    icon: <CheckIcon size={20} />, 
    title: "Connected",
    iconVariant: "success"
  }}
>
  <p>Secure connection established</p>
</Card>

// Conversation card (clickable)
<Card 
  clickable
  onClick={() => openChat(conversation.id)}
>
  <div className="conversation-preview">
    <h4>{conversation.name}</h4>
    <p>{conversation.lastMessage}</p>
  </div>
</Card>
```

---

## Badge

Small status indicator with semantic color variants.

### Import

```tsx
import Badge from '@/components/ui/Badge';
```

### Props

```tsx
interface BadgeProps {
  children: ReactNode;
  variant?: "default" | "success" | "danger" | "warning" | "info";
  dot?: boolean;
  compact?: boolean;
  style?: CSSProperties;
  id?: string;
}
```

### Variants

```tsx
<Badge variant="default">Default</Badge>
<Badge variant="success">Online</Badge>
<Badge variant="danger">Error</Badge>
<Badge variant="warning">Warning</Badge>
<Badge variant="info">Info</Badge>
```

**Colors:**
- `default`: Muted gray
- `success`: Green (online, verified, success states)
- `danger`: Red (offline, error, critical)
- `warning`: Amber (caution, pending)
- `info`: Indigo (informational)

---

### With Animated Dot

```tsx
<Badge variant="success" dot>
  Online
</Badge>
```

**Visual:** Pulsing dot animation to the left of text.

**Use cases:**
- Connection status
- Live indicators
- Real-time states

---

### Examples

```tsx
// Connection status
<Badge variant="success" dot>
  Connected
</Badge>

// Message count
<Badge variant="info">
  3 unread
</Badge>

// Error indicator
<Badge variant="danger">
  Connection failed
</Badge>

// Verification badge
<Badge variant="success">
  <VerifiedIcon size={14} />
  Verified
</Badge>
```

---

## Modal

Accessible dialog component with focus trapping and keyboard navigation.

### Import

```tsx
import Modal from '@/components/ui/Modal';
```

### Props

```tsx
interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  footer?: ReactNode;
  maxWidth?: number;
}
```

### Basic Usage

```tsx
const [open, setOpen] = useState(false);

<Modal 
  open={open}
  onClose={() => setOpen(false)}
  title="Confirm Action"
>
  <p>Are you sure you want to proceed?</p>
</Modal>
```

---

### With Footer

```tsx
<Modal 
  open={open}
  onClose={() => setOpen(false)}
  title="Delete Conversation"
  footer={
    <>
      <Button variant="secondary" onClick={() => setOpen(false)}>
        Cancel
      </Button>
      <Button variant="danger" onClick={handleDelete}>
        Delete
      </Button>
    </>
  }
>
  <p>This action cannot be undone.</p>
</Modal>
```

---

### Custom Width

```tsx
<Modal 
  open={open}
  onClose={() => setOpen(false)}
  title="Large Content"
  maxWidth={800}
>
  <div>Wide content here</div>
</Modal>
```

**Default:** 560px

---

### Accessibility Features

**Focus Management:**
- Captures focus when opened
- Focus trap (Tab cycles through modal elements)
- Restores focus to trigger element on close
- Auto-focuses first interactive element

**Keyboard Navigation:**
- `Escape`: Close modal
- `Tab`: Navigate forward through focusable elements
- `Shift+Tab`: Navigate backward

**ARIA Attributes:**
- `role="dialog"`
- `aria-modal="true"`
- `aria-label` set to modal title

---

### Examples

```tsx
// Confirmation modal
<Modal 
  open={confirmOpen}
  onClose={() => setConfirmOpen(false)}
  title="Confirm Deletion"
  footer={
    <>
      <Button variant="secondary" onClick={() => setConfirmOpen(false)}>
        Cancel
      </Button>
      <Button variant="danger" onClick={handleConfirm}>
        Delete
      </Button>
    </>
  }
>
  <p>Are you sure you want to delete this conversation?</p>
  <p>This action cannot be undone.</p>
</Modal>

// Fingerprint verification modal
<Modal 
  open={verifyOpen}
  onClose={() => setVerifyOpen(false)}
  title="Verify Fingerprint"
  maxWidth={640}
>
  <div className="fingerprint-display">
    <code>{fingerprint}</code>
  </div>
  <p>Verify this fingerprint matches on both devices</p>
</Modal>
```

---

## Select

Custom-styled dropdown select component.

### Import

```tsx
import Select from '@/components/ui/Select';
```

### Props

```tsx
interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps {
  options: SelectOption[];
  placeholder?: string;
  error?: string;
  compact?: boolean;
  fullWidth?: boolean;
  value?: string;
  onChange?: (e: React.ChangeEvent<HTMLSelectElement>) => void;
  className?: string;
  // + all native select HTML attributes
}
```

### Basic Usage

```tsx
<Select 
  options={[
    { value: 'auto', label: 'Auto' },
    { value: 'manual', label: 'Manual' }
  ]}
  value={mode}
  onChange={(e) => setMode(e.target.value)}
/>
```

---

### With Placeholder

```tsx
<Select 
  placeholder="Select an option..."
  options={options}
  value={selected}
  onChange={(e) => setSelected(e.target.value)}
/>
```

**Behavior:** Placeholder is disabled option, cannot be reselected.

---

### Compact Mode

```tsx
<Select 
  compact
  options={portOptions}
  value={port}
  onChange={(e) => setPort(e.target.value)}
/>
```

---

### Examples

```tsx
// Retention policy selector
<Select 
  placeholder="Choose retention period..."
  options={[
    { value: '1h', label: '1 Hour' },
    { value: '24h', label: '24 Hours' },
    { value: '7d', label: '7 Days' },
    { value: 'forever', label: 'Forever' }
  ]}
  value={retention}
  onChange={(e) => setRetention(e.target.value)}
/>

// Network configuration
<Select 
  compact
  options={[
    { value: 'auto', label: 'Auto-detect' },
    { value: 'upnp', label: 'UPnP' },
    { value: 'manual', label: 'Manual' }
  ]}
  value={natMode}
  onChange={(e) => setNatMode(e.target.value)}
/>
```

---

## Toast

Temporary notification system with auto-dismiss.

### Import

```tsx
import { ToastContainer, type ToastData } from '@/components/ui/Toast';
```

### Types

```tsx
interface ToastData {
  id: string;
  message: string;
  type: "success" | "error" | "warning" | "info";
  duration?: number; // milliseconds, default 4000
}
```

### Usage with Context

Toast system is managed via `AppContext`:

```tsx
import { useApp } from '@/context/AppContext';

function MyComponent() {
  const { showToast } = useApp();
  
  const handleSuccess = () => {
    showToast('Connection established!', 'success');
  };
  
  const handleError = () => {
    showToast('Failed to connect', 'error');
  };
  
  return <Button onClick={handleSuccess}>Connect</Button>;
}
```

---

### Toast Types

```tsx
// Success
showToast('Message sent successfully', 'success');

// Error
showToast('Connection failed', 'error');

// Warning
showToast('Weak passphrase detected', 'warning');

// Info
showToast('New message received', 'info');
```

---

### Custom Duration

```tsx
// Show for 10 seconds
showToast('Important message', 'info', 10000);

// Show for 2 seconds
showToast('Copied!', 'success', 2000);
```

---

### Visual Features

- **Position:** Bottom-right corner
- **Animation:** Slide in from right, fade out
- **Progress Bar:** Animated countdown indicator
- **Auto-dismiss:** Closes after duration expires
- **Manual Dismiss:** Click anywhere on toast or × button
- **Stacking:** Multiple toasts stack vertically

---

### Accessibility

- `role="alert"` for screen reader announcements
- `aria-live="assertive"` for immediate notification
- Keyboard accessible dismiss button
- Click-to-dismiss on entire toast

---

### Examples

```tsx
// Copy to clipboard confirmation
const handleCopy = () => {
  navigator.clipboard.writeText(text);
  showToast('Copied to clipboard', 'success', 2000);
};

// Network error handling
try {
  await connectToServer();
  showToast('Connected successfully', 'success');
} catch (error) {
  showToast(error.message, 'error');
}

// Validation warning
if (passphraseStrength < 50) {
  showToast('Consider using a stronger passphrase', 'warning', 6000);
}
```

---

## LoadingSpinner

Animated loading indicator with optional overlay.

### Import

```tsx
import LoadingSpinner from '@/components/ui/LoadingSpinner';
```

### Props

```tsx
interface LoadingSpinnerProps {
  label?: string;
  size?: "sm" | "md" | "lg";
  overlay?: boolean;
}
```

### Sizes

```tsx
<LoadingSpinner size="sm" />
<LoadingSpinner size="md" /> {/* default */}
<LoadingSpinner size="lg" />
```

**Dimensions:**
- Small: 20px
- Medium: 32px
- Large: 48px

---

### With Label

```tsx
<LoadingSpinner 
  size="lg"
  label="Loading conversation..."
/>
```

---

### Overlay Mode

Creates full-screen overlay with centered spinner.

```tsx
<LoadingSpinner 
  overlay
  size="lg"
  label="Connecting..."
/>
```

**Use cases:**
- Page loading states
- Full-screen blocking operations
- Initial app load

---

### Examples

```tsx
// Inline loading in card
<Card>
  {loading ? (
    <LoadingSpinner label="Fetching data..." />
  ) : (
    <div>{data}</div>
  )}
</Card>

// Full-page overlay
{isConnecting && (
  <LoadingSpinner 
    overlay
    size="lg"
    label="Establishing secure connection..."
  />
)}

// Small inline spinner
<Button disabled={loading}>
  {loading && <LoadingSpinner size="sm" />}
  {loading ? 'Sending...' : 'Send'}
</Button>
```

---

## Icons

Tree-shakeable SVG icon system.

### Import

```tsx
// Individual imports (recommended)
import { ShieldIcon } from '@/components/ui/icons/ShieldIcon';
import { LockIcon } from '@/components/ui/icons/LockIcon';

// Barrel import (convenience)
import { ShieldIcon, LockIcon } from '@/components/ui/Icons';
```

### Icon Props

```tsx
interface IconProps {
  size?: number;
  color?: string;
  className?: string;
}
```

### Basic Usage

```tsx
<ShieldIcon size={24} />
<LockIcon size={20} color="var(--color-accent)" />
```

---

### Available Icons

**Security & Identity:**
- `ShieldIcon` - Security, protection
- `LockIcon` - Encryption, locked state
- `UnlockIcon` - Unlocked state
- `KeyIcon` - Keys, credentials
- `VerifiedIcon` - Verified status, checkmark badge

**Navigation & Actions:**
- `HomeIcon` - Home, dashboard
- `GearIcon` - Settings, configuration
- `SearchIcon` - Search functionality
- `ArrowLeftIcon` - Back navigation
- `ArrowDownIcon` - Dropdown, expand
- `ChevronDownIcon` - Dropdown indicator
- `CloseIcon` - Close, dismiss, delete
- `CheckIcon` - Confirm, success

**Communication:**
- `MessageIcon` - Messages, chat
- `SendIcon` - Send message
- `AttachIcon` - Attach file
- `FileIcon` - File, document

**Connection & Status:**
- `LinkIcon` - Link, connection
- `GlobeIcon` - Network, internet
- `WifiIcon` - Wi-Fi, connectivity
- `OnlineDot` - Online status indicator
- `OfflineDot` - Offline status indicator

**Utility:**
- `PlusIcon` - Add, create new
- `CopyIcon` - Copy to clipboard
- `TrashIcon` - Delete, remove
- `EyeIcon` - Show, reveal
- `EyeOffIcon` - Hide, conceal
- `AlertTriangleIcon` - Warning, caution
- `InfoIcon` - Information, help

---

### Size Guidelines

**Recommended Sizes:**
- 14px: Inline with small text, badges
- 16px: Inline with body text, compact UI
- 18px: Input icons, small buttons
- 20px: Card headers, standard buttons
- 24px: Large buttons, headings
- 32px+: Hero icons, empty states

**Current Inconsistency:** Icon sizes vary (16-22px). Phase 2 will standardize to 16/20/24px system.

---

### Color Usage

```tsx
// Use design tokens
<CheckIcon 
  size={18} 
  color="var(--color-success)" 
/>

// Inherit from parent
<button style={{ color: 'var(--color-accent)' }}>
  <PlusIcon size={20} /> {/* Inherits accent color */}
</button>
```

---

### Examples

```tsx
// Button with icon
<Button icon={<SendIcon size={18} />}>
  Send Message
</Button>

// Input with search icon
<Input 
  icon={<SearchIcon size={18} />}
  placeholder="Search..."
/>

// Status badge with icon
<Badge variant="success">
  <VerifiedIcon size={14} />
  Verified
</Badge>

// Card header with icon
<Card 
  header={{ 
    icon: <ShieldIcon size={20} />, 
    title: "Security" 
  }}
/>

// Toggle visibility
<Button 
  variant="icon"
  onClick={() => setShow(!show)}
  aria-label={show ? 'Hide' : 'Show'}
>
  {show ? <EyeOffIcon size={20} /> : <EyeIcon size={20} />}
</Button>
```

---

## Best Practices

### Composition Patterns

**Do:** Build complex UIs by composing simple components
```tsx
<Card header={{ icon: <KeyIcon />, title: "Identity" }}>
  <Input 
    mono
    readOnly
    value={fingerprint}
    icon={<CopyIcon />}
  />
  <Button onClick={copyFingerprint}>
    Copy to Clipboard
  </Button>
</Card>
```

**Don't:** Create overly complex single components
```tsx
<MegaCard 
  showHeader
  headerType="identity"
  hasInput
  inputType="mono"
  hasButton
  buttonText="Copy"
/>
```

---

### Accessibility

**Always:**
- Provide `aria-label` for icon-only buttons
- Use semantic HTML (`<button>`, not `<div onClick>`)
- Ensure 4.5:1 color contrast for text
- Support keyboard navigation
- Test with screen readers

**Example:**
```tsx
<Button 
  variant="icon"
  onClick={handleClose}
  aria-label="Close dialog"
>
  <CloseIcon size={20} />
</Button>
```

---

### Performance

**Do:** Import icons individually for tree-shaking
```tsx
import { ShieldIcon } from '@/components/ui/icons/ShieldIcon';
```

**Don't:** Import entire icon library
```tsx
import * as Icons from '@/components/ui/Icons';
```

---

### Controlled Components

**Always use controlled inputs:**
```tsx
// ✓ Good
<Input 
  value={state}
  onChange={(e) => setState(e.target.value)}
/>

// ✗ Avoid
<Input defaultValue="text" />
```

---

### Error Handling

**Provide clear, actionable error messages:**
```tsx
// ✓ Good
<Input 
  error="Passphrase must be at least 12 characters"
/>

// ✗ Avoid
<Input 
  error="Invalid input"
/>
```

---

### Loading States

**Always show feedback for async operations:**
```tsx
<Button loading={isSubmitting} onClick={handleSubmit}>
  {isSubmitting ? 'Submitting...' : 'Submit'}
</Button>
```

---

## Migration Notes

### Phase 2 Changes (Upcoming)

**Icon Sizing Standardization:**
Current icons use inconsistent sizes (16-22px). Phase 2 will standardize to:
- Small: 16px
- Medium: 20px
- Large: 24px

**Component CSS Splitting:**
`components.css` (2052 lines) will be split into individual files:
- `button.css`
- `input.css`
- `card.css`
- etc.

**Component API Improvements:**
Minor prop API changes for consistency across components.

---

## Related Documentation

- [Design System](./design-system.md) - Design tokens and guidelines
- [Icon System Documentation](./icon-system.md) - Detailed icon reference
- [Accessibility Guidelines](./accessibility.md) - WCAG compliance

---

**Last Updated:** Phase 1 Implementation
**Maintained By:** M2M Development Team
**Version:** 1.0
