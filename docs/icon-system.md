# M2M Icon System Documentation

## Overview

M2M uses a custom SVG icon system optimized for tree-shaking, accessibility, and consistent styling. All icons are stroke-based with rounded line caps for a modern, cohesive aesthetic.

**Key Features:**
- 32 custom SVG icons
- Tree-shakeable (import only what you use)
- Configurable size and color
- Consistent stroke weight (1.5px)
- TypeScript support
- Accessibility-friendly

---

## Architecture

### File Structure

```
src/components/ui/icons/
├── types.ts                 # Shared TypeScript types
├── ShieldIcon.tsx           # Individual icon components
├── LockIcon.tsx
├── ... (32 total icons)
└── Icons.tsx (deprecated)   # Barrel export for compatibility
```

### Icon Props Interface

```tsx
interface IconProps {
  size?: number;        // Icon dimensions (width/height)
  color?: string;       // Stroke/fill color
  className?: string;   // Additional CSS classes
}
```

---

## Usage

### Importing Icons

**Recommended: Direct imports (best for tree-shaking)**
```tsx
import { ShieldIcon } from '@/components/ui/icons/ShieldIcon';
import { LockIcon } from '@/components/ui/icons/LockIcon';
```

**Legacy: Barrel import (convenience)**
```tsx
import { ShieldIcon, LockIcon } from '@/components/ui/Icons';
```

### Basic Usage

```tsx
// Default size (24px), inherits color from parent
<ShieldIcon />

// Custom size
<ShieldIcon size={20} />

// Custom color
<ShieldIcon size={24} color="var(--color-accent)" />

// With CSS class
<ShieldIcon size={18} className="custom-icon" />
```

---

## Icon Inventory

### Security & Identity

#### ShieldIcon
**Usage:** Security features, protection, verified states  
**Default Size:** 24px  
**Type:** Stroke-based outline

```tsx
<ShieldIcon size={20} />
```

**Use Cases:**
- Security settings header
- Protection indicators
- Verified/trusted badges
- Encryption status

---

#### LockIcon
**Usage:** Encrypted states, locked content, security features  
**Default Size:** 24px  
**Type:** Stroke-based outline

```tsx
<LockIcon size={18} />
```

**Use Cases:**
- Passphrase inputs
- Encrypted connections
- Locked conversations
- Security indicators

---

#### UnlockIcon
**Usage:** Unlocked states, decryption, open access  
**Default Size:** 24px  
**Type:** Stroke-based outline

```tsx
<UnlockIcon size={18} />
```

**Use Cases:**
- Unlock vault actions
- Decrypted state indicators
- Open access features

---

#### KeyIcon
**Usage:** Cryptographic keys, credentials, authentication  
**Default Size:** 24px  
**Type:** Stroke-based outline

```tsx
<KeyIcon size={20} />
```

**Use Cases:**
- Key generation
- Identity cards
- Credential management
- Authentication flows

---

#### VerifiedIcon
**Usage:** Verification, confirmation, success with badge  
**Default Size:** 24px  
**Type:** Stroke-based with checkmark

```tsx
<VerifiedIcon size={16} color="var(--color-success)" />
```

**Use Cases:**
- Verified fingerprints
- Confirmed connections
- Success badges
- Trust indicators

---

### Navigation & Actions

#### HomeIcon
**Usage:** Home view, dashboard navigation  
**Default Size:** 24px  
**Type:** Stroke-based outline

```tsx
<HomeIcon size={20} />
```

---

#### GearIcon
**Usage:** Settings, configuration, preferences  
**Default Size:** 24px  
**Type:** Stroke-based outline

```tsx
<GearIcon size={20} />
```

**Use Cases:**
- Settings navigation
- Configuration panels
- Preferences menus

---

#### SearchIcon
**Usage:** Search functionality, filters, find features  
**Default Size:** 24px  
**Type:** Stroke-based outline

```tsx
<SearchIcon size={18} />
```

**Common Pattern:**
```tsx
<Input 
  icon={<SearchIcon size={18} />}
  placeholder="Search conversations..."
/>
```

---

#### ArrowLeftIcon
**Usage:** Back navigation, previous actions  
**Default Size:** 24px  
**Type:** Stroke-based outline

```tsx
<ArrowLeftIcon size={20} />
```

**Use Cases:**
- Back buttons
- Previous navigation
- Return to previous view

---

#### ArrowDownIcon
**Usage:** Expand/collapse, scroll indicators  
**Default Size:** 24px  
**Type:** Stroke-based outline

```tsx
<ArrowDownIcon size={16} />
```

---

#### ChevronDownIcon
**Usage:** Dropdown indicators, expandable sections  
**Default Size:** 24px  
**Type:** Stroke-based outline

```tsx
<ChevronDownIcon size={14} />
```

**Use Cases:**
- Select dropdowns
- Accordion headers
- Expandable menus

---

#### CloseIcon
**Usage:** Close actions, dismiss, delete  
**Default Size:** 24px  
**Type:** Stroke-based X

```tsx
<CloseIcon size={18} />
```

**Use Cases:**
- Modal close buttons
- Toast dismissal
- Clear input fields
- Remove items

**Common Pattern:**
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

#### CheckIcon
**Usage:** Confirmation, success, completed states  
**Default Size:** 24px  
**Type:** Stroke-based checkmark

```tsx
<CheckIcon size={18} color="var(--color-success)" />
```

**Use Cases:**
- Success messages
- Completed tasks
- Confirmation indicators
- Selected states

---

#### PlusIcon
**Usage:** Add actions, create new, expand  
**Default Size:** 24px  
**Type:** Stroke-based plus

```tsx
<PlusIcon size={20} />
```

**Use Cases:**
- Add new conversation
- Create invite
- Add attachments

**Common Pattern:**
```tsx
<Button icon={<PlusIcon size={18} />}>
  New Connection
</Button>
```

---

### Communication

#### MessageIcon
**Usage:** Messages, chat, conversations  
**Default Size:** 24px  
**Type:** Stroke-based speech bubble

```tsx
<MessageIcon size={20} />
```

**Use Cases:**
- Chat navigation
- Message indicators
- Conversation headers

---

#### SendIcon
**Usage:** Send message, submit, dispatch  
**Default Size:** 24px  
**Type:** Stroke-based paper plane

```tsx
<SendIcon size={18} />
```

**Common Pattern:**
```tsx
<Button 
  icon={<SendIcon size={18} />}
  onClick={sendMessage}
>
  Send
</Button>
```

---

#### AttachIcon
**Usage:** File attachments, add files  
**Default Size:** 24px  
**Type:** Stroke-based paperclip

```tsx
<AttachIcon size={20} />
```

**Use Cases:**
- Attach file button
- File upload indicators
- Attachment management

---

#### FileIcon
**Usage:** File representations, documents  
**Default Size:** 24px  
**Type:** Stroke-based document

```tsx
<FileIcon size={20} />
```

**Use Cases:**
- File transfer previews
- Document indicators
- Attachment icons

---

### Connection & Status

#### LinkIcon
**Usage:** Links, connections, invite URLs  
**Default Size:** 24px  
**Type:** Stroke-based chain link

```tsx
<LinkIcon size={20} />
```

**Use Cases:**
- Generate invite link
- Connection management
- URL indicators

---

#### GlobeIcon
**Usage:** Network, internet, global settings  
**Default Size:** 24px  
**Type:** Stroke-based globe

```tsx
<GlobeIcon size={20} />
```

**Use Cases:**
- Network settings
- Internet connectivity
- Global configurations

---

#### WifiIcon
**Usage:** Wi-Fi, wireless connectivity, network status  
**Default Size:** 24px  
**Type:** Stroke-based Wi-Fi waves

```tsx
<WifiIcon size={20} />
```

**Use Cases:**
- Network diagnostics
- Connection status
- Connectivity indicators

---

#### OnlineDot
**Usage:** Online status indicator  
**Default Size:** 8px  
**Type:** Filled circle

```tsx
<OnlineDot size={8} color="var(--color-success)" />
```

**Special Properties:**
- Smaller default size (8px vs 24px)
- Filled (not stroke-based)
- Typically animated with pulse

**Common Pattern:**
```tsx
<Badge variant="success" dot>
  <OnlineDot size={8} />
  Online
</Badge>
```

---

#### OfflineDot
**Usage:** Offline status indicator  
**Default Size:** 8px  
**Type:** Filled circle

```tsx
<OfflineDot size={8} color="var(--color-text-muted)" />
```

**Use Cases:**
- Offline status
- Disconnected state
- Inactive indicators

---

### Utility

#### CopyIcon
**Usage:** Copy to clipboard actions  
**Default Size:** 24px  
**Type:** Stroke-based documents

```tsx
<CopyIcon size={18} />
```

**Common Pattern:**
```tsx
<Button 
  variant="ghost"
  icon={<CopyIcon size={16} />}
  onClick={copyToClipboard}
>
  Copy
</Button>
```

---

#### TrashIcon
**Usage:** Delete actions, remove items  
**Default Size:** 24px  
**Type:** Stroke-based trash can

```tsx
<TrashIcon size={18} color="var(--color-danger)" />
```

**Common Pattern:**
```tsx
<Button 
  variant="danger"
  icon={<TrashIcon size={18} />}
  onClick={handleDelete}
>
  Delete
</Button>
```

---

#### EyeIcon
**Usage:** Show/reveal content, visibility on  
**Default Size:** 24px  
**Type:** Stroke-based eye

```tsx
<EyeIcon size={20} />
```

**Use Cases:**
- Show password toggle
- Reveal hidden content
- Preview mode

---

#### EyeOffIcon
**Usage:** Hide content, visibility off  
**Default Size:** 24px  
**Type:** Stroke-based eye with slash

```tsx
<EyeOffIcon size={20} />
```

**Common Pattern (Password Toggle):**
```tsx
const [show, setShow] = useState(false);

<Button 
  variant="icon"
  onClick={() => setShow(!show)}
  aria-label={show ? 'Hide password' : 'Show password'}
>
  {show ? <EyeOffIcon size={20} /> : <EyeIcon size={20} />}
</Button>
```

---

#### AlertTriangleIcon
**Usage:** Warnings, caution, important alerts  
**Default Size:** 24px  
**Type:** Stroke-based triangle with exclamation

```tsx
<AlertTriangleIcon size={20} color="var(--color-warning)" />
```

**Use Cases:**
- Warning messages
- Caution indicators
- Error states

---

#### InfoIcon
**Usage:** Information, help, tooltips  
**Default Size:** 24px  
**Type:** Stroke-based circle with 'i'

```tsx
<InfoIcon size={18} color="var(--color-accent-bright)" />
```

**Use Cases:**
- Info tooltips
- Help indicators
- Informational messages

---

## Size Guidelines

### Recommended Sizes

```tsx
// Inline with text
<span>Message <MessageIcon size={14} /></span>

// Small UI (badges, compact buttons)
<Badge><VerifiedIcon size={14} /></Badge>

// Input fields
<Input icon={<SearchIcon size={18} />} />

// Standard buttons
<Button icon={<SendIcon size={18} />}>Send</Button>

// Card headers
<Card header={{ icon: <ShieldIcon size={20} /> }} />

// Large buttons
<Button icon={<PlusIcon size={24} />} size="lg">Create</Button>

// Hero sections
<ShieldIcon size={48} />
```

### Size Chart

| Context | Recommended Size | Example |
|---------|------------------|---------|
| Inline text | 14px | Badge icons |
| Compact UI | 16px | Small buttons, tight layouts |
| Inputs | 18px | Search, form fields |
| Buttons | 18-20px | Standard CTAs |
| Headers | 20-24px | Card headers, titles |
| Large actions | 24px | Primary buttons |
| Empty states | 32-48px | No content indicators |
| Hero sections | 48px+ | Feature highlights |

---

## Color Patterns

### Using Design Tokens

```tsx
// Semantic colors
<CheckIcon color="var(--color-success)" />
<TrashIcon color="var(--color-danger)" />
<AlertTriangleIcon color="var(--color-warning)" />
<InfoIcon color="var(--color-info)" />

// Text colors
<ShieldIcon color="var(--color-text-primary)" />
<GearIcon color="var(--color-text-secondary)" />
<CloseIcon color="var(--color-text-muted)" />

// Accent colors
<LinkIcon color="var(--color-accent)" />
<KeyIcon color="var(--color-accent-bright)" />
```

### Inheriting Color

Icons default to `currentColor`, inheriting from parent:

```tsx
<button style={{ color: 'var(--color-accent)' }}>
  <PlusIcon size={20} /> {/* Automatically inherits accent color */}
  Add Item
</button>
```

---

## Accessibility

### Icon-Only Buttons

Always provide `aria-label` for buttons with only icons:

```tsx
// ✓ Good
<Button 
  variant="icon"
  onClick={handleClose}
  aria-label="Close dialog"
>
  <CloseIcon size={20} />
</Button>

// ✗ Bad
<Button variant="icon" onClick={handleClose}>
  <CloseIcon size={20} />
</Button>
```

---

### Decorative Icons

Icons paired with text are decorative and should not be announced:

```tsx
// Text provides context, icon is visual enhancement
<Button icon={<SendIcon size={18} />}>
  Send Message
</Button>
```

---

### Status Icons

Use semantic colors and proper labeling:

```tsx
<Badge variant="success" aria-label="Online status">
  <OnlineDot size={8} />
  Online
</Badge>
```

---

## Technical Details

### SVG Attributes

All stroke-based icons use consistent attributes:

```tsx
<svg 
  width={size} 
  height={size} 
  viewBox="0 0 24 24"
  fill="none"
  stroke={color}
  strokeWidth="1.5"
  strokeLinecap="round"
  strokeLinejoin="round"
>
  {/* paths */}
</svg>
```

**Consistency:**
- ViewBox: `0 0 24 24` (normalized coordinate system)
- Stroke width: `1.5` (optimal for clarity at various sizes)
- Line caps: `round` (modern, friendly aesthetic)
- Line joins: `round` (smooth corners)

---

### Status Dot Icons

Status dots (OnlineDot, OfflineDot) use filled circles:

```tsx
<svg width={size} height={size} viewBox="0 0 8 8">
  <circle cx="4" cy="4" r="4" fill={color} />
</svg>
```

**Differences:**
- ViewBox: `0 0 8 8` (smaller coordinate system)
- Fill-based (not stroke)
- Default size: 8px (not 24px)

---

## Known Issues & Roadmap

### Current Inconsistencies (Phase 2 Fixes)

**Icon Size Usage:**
Current codebase uses inconsistent sizes:
- Some icons at 16px
- Some at 18px
- Some at 20px
- Some at 22px

**Standardization Plan (Phase 2):**
- Small: 16px
- Medium: 20px (default)
- Large: 24px

**Action Items:**
1. Audit all icon usage in codebase
2. Standardize to 16/20/24px system
3. Update component examples
4. Document migration guide

---

### Missing Icons

Consider adding in future iterations:
- Download icon
- Upload icon
- Calendar icon
- Clock icon
- Bell (notifications)
- User/profile icon
- More/options (three dots)

---

## Performance Optimization

### Tree-Shaking

Icons are tree-shakeable when imported directly:

```tsx
// ✓ Only ShieldIcon included in bundle
import { ShieldIcon } from '@/components/ui/icons/ShieldIcon';

// ⚠️ May include all icons (depends on bundler)
import { ShieldIcon } from '@/components/ui/Icons';
```

**Recommendation:** Use direct imports in production code.

---

### Bundle Size

Each icon component:
- ~200-400 bytes (minified + gzipped)
- Zero runtime overhead
- No dependencies

Total icon system:
- ~12 KB for all 32 icons (uncompressed)
- ~3-4 KB (minified + gzipped)

---

## Testing

### Visual Regression Testing

```tsx
// Test all sizes
<div>
  <ShieldIcon size={14} />
  <ShieldIcon size={18} />
  <ShieldIcon size={24} />
</div>

// Test color inheritance
<div style={{ color: 'var(--color-accent)' }}>
  <ShieldIcon />
</div>

// Test custom colors
<ShieldIcon color="var(--color-success)" />
```

---

### Accessibility Testing

```tsx
// Icon-only button
<Button 
  variant="icon"
  aria-label="Settings"
>
  <GearIcon size={20} />
</Button>

// Icon with text (decorative)
<Button icon={<SendIcon size={18} />}>
  Send
</Button>
```

**Checklist:**
- [ ] Icon-only buttons have aria-label
- [ ] Status icons use semantic colors
- [ ] Icons inherit color properly
- [ ] Icons scale correctly at 200% zoom
- [ ] SVG accessible to screen readers (role="img" if needed)

---

## Examples

### Complete Component Patterns

#### Navigation Button
```tsx
<Button 
  variant="ghost"
  icon={<ArrowLeftIcon size={20} />}
  onClick={() => navigate(-1)}
>
  Back
</Button>
```

#### Status Badge
```tsx
<Badge variant="success" dot>
  <OnlineDot size={8} />
  Connected
</Badge>
```

#### Search Input
```tsx
<Input 
  icon={<SearchIcon size={18} />}
  placeholder="Search conversations..."
  value={query}
  onChange={(e) => setQuery(e.target.value)}
/>
```

#### Action Card
```tsx
<Card 
  header={{ 
    icon: <KeyIcon size={20} />, 
    title: "Identity",
    iconVariant: "accent"
  }}
>
  <p>Your public key fingerprint</p>
</Card>
```

#### Icon Button Toolbar
```tsx
<div className="toolbar">
  <Button variant="icon" aria-label="Attach file">
    <AttachIcon size={20} />
  </Button>
  <Button variant="icon" aria-label="Send message">
    <SendIcon size={20} />
  </Button>
  <Button variant="icon" aria-label="Settings">
    <GearIcon size={20} />
  </Button>
</div>
```

---

## Migration Guide

### From Icon Library (e.g., Feather, Heroicons)

If migrating from an external icon library:

1. **Find equivalent M2M icon** (see inventory above)
2. **Update imports:**
   ```tsx
   // Before
   import { Shield } from 'react-feather';
   
   // After
   import { ShieldIcon } from '@/components/ui/icons/ShieldIcon';
   ```
3. **Update usage:**
   ```tsx
   // Before
   <Shield size={20} />
   
   // After
   <ShieldIcon size={20} />
   ```

---

## Contributing

### Adding New Icons

1. Create new file: `src/components/ui/icons/NewIcon.tsx`
2. Follow existing pattern:
   ```tsx
   import { type IconProps } from "./types";
   
   export function NewIcon({ 
     size = 24, 
     color = "currentColor", 
     className 
   }: IconProps) {
     return (
       <svg 
         className={className}
         width={size} 
         height={size} 
         viewBox="0 0 24 24"
         fill="none"
         stroke={color}
         strokeWidth="1.5"
         strokeLinecap="round"
         strokeLinejoin="round"
       >
         {/* SVG paths */}
       </svg>
     );
   }
   ```
3. Export from `Icons.tsx` for compatibility
4. Document in this file

---

## Related Documentation

- [Design System](./design-system.md) - Design tokens and guidelines
- [Component Guide](./component-guide.md) - Component usage examples
- [WCAG Audit](./wcag-contrast-audit.md) - Accessibility compliance

---

**Last Updated:** Phase 1 Implementation  
**Maintained By:** M2M Development Team  
**Total Icons:** 32  
**Status:** Documentation complete, standardization pending Phase 2
