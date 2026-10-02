# FileTransfer — Implementation Prompt

## Mission

Implement the file transfer UI components for sending and receiving files in ChatView. This includes file request banners for incoming transfers and progress displays for active transfers.

## Scope

Covers file transfer UI including:
- File request banner (incoming): filename, size, Accept/Reject buttons
- File transfer progress display: filename, size, progress bar, speed, ETA
- Cancel button on active transfers
- Error states (failed, cancelled)
- Completion state with success indicator

Does NOT cover: The file transfer backend (chunking, ACK protocol, hashing), file dialog handling.

## Files Expected to Be Modified

- `src/components/FileRequestBanner.tsx` — Incoming request component
- `src/components/FileTransferProgress.tsx` — Active transfer component
- `src/styles/components/utilities.css` — Component styles

## Components to Reuse

- **Button** (Section 2.1) — Accept, Reject, Cancel actions
- **ProgressBar** (Section 2.8) — Transfer progress (default 8px)
- **Badge** (Section 2.5) — Status labels

## Components to Create

- **FileRequestBanner** — Incoming file notification with actions
- **FileTransferProgress** — Active transfer progress display

## Layout Hierarchy

**File Request Banner (52px height):**
```
┌──────────────────────────────────────┐
│ 📄  report.pdf     2.4 MB            │
│                       [Accept] [Reject]│
└──────────────────────────────────────┘
```

**File Transfer Progress (72px height):**
```
┌──────────────────────────────────────┐
│ 📄  photo.jpg              4.2 MB    │
│ ████████████████░░░░░░  65%          │
│ transferring      2.1 MB/s · 12s remaining │
└──────────────────────────────────────┘
```

## Design Implementation Requirements

### Specs

- Banner height: 52px, padding 8px 32px
- Progress display height: 72px, padding 8px 32px
- ProgressBar height: 8px (default), border-radius: --radius-full
- Label: --text-sm, --color-text-secondary
- Speed/ETA: --text-xs, --color-text-muted

### Colors

- Progress track: --color-bg-input
- Progress fill (default): --color-accent
- Progress fill (success): --color-success (#10b981)
- Progress fill (error): --color-danger (#ef4444)
- Progress fill (warning): --color-warning (#f59e0b)

### Error Messages

From Design Bible Part 3 Section 21.4:

| ID | Trigger | Message | Type |
|----|---------|---------|------|
| F-001 | File too large | "File exceeds the maximum transfer size." | error toast, 6s |
| F-002 | File not found | "File not found at path: {path}" | error toast, 6s |
| F-004 | Cancelled by peer | "{filename} was cancelled by the peer." | info toast, 4s |
| F-005 | Cancelled by self | "{filename} cancelled." | info toast, 4s |
| F-006 | Chunk timeout | "Chunk send timed out. Retrying ({n}/3)..." | warning inline |
| F-007 | Hash mismatch | "File integrity check failed." | error toast, 8s |
| F-008 | Complete (sender) | "{filename} sent successfully." | success toast, 4s |
| F-009 | Complete (receiver) | "{filename} downloaded successfully." | success toast, 4s |
| F-013 | Disk write error | "Failed to save file: {error}. Check disk space and permissions." | error toast, 8s |

### Database

From Design Bible Part 3 Section 24.3 (transfers table):
- transfer_id (UUID), peer_key_hex, filename, total_size, direction, state
- chunks_acked, chunks_total, local_path, speed_bytes_per_sec, error

## States

| State | Visual |
|-------|--------|
| Request (incoming) | Banner with filename, size, Accept/Reject |
| Transferring | Progress bar animating (width transition 300ms), speed/ETA updating |
| Completed | Progress bar full (success color), checkmark icon |
| Failed | Progress bar red (danger color), "Failed" label |
| Cancelled | Progress bar grey, "Cancelled" label |
| Paused (future) | Progress bar with pause icon |

## Acceptance Criteria

- [ ] Incoming file request shows banner with filename, size, Accept/Reject
- [ ] Active transfer shows filename, size, progress bar, percentage
- [ ] Speed and ETA shown below progress bar
- [ ] Progress bar animates width transition (300ms ease-out-expo)
- [ ] Cancel button available on active transfers
- [ ] Completed state shows success toast + full progress bar
- [ ] Failed state shows red progress bar + error toast
- [ ] Cancelled state shows grey progress bar + info toast
- [ ] Chunk retry shows warning with retry count
- [ ] All error messages match spec

## Self-Review Checklist

- [ ] Follows Design Bible Section 3.4, 4.5
- [ ] ProgressBar specs from Section 2.8
- [ ] Error messages from Section 21.4
- [ ] Database schema from Section 24.3
