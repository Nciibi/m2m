/// Estimate passphrase entropy in bits using character-pool + pattern detection.
///
/// Uses a character-pool base model, then applies pattern-based penalties:
/// - Sequential characters ("abcd", "1234") → penalize
/// - Repeating characters ("aaa", "1111") → penalize
/// - Keyboard patterns ("qwerty", "asdf") → penalize
/// - Short length (< 12 chars) → heavy penalty
///
/// Implements NIST SP 800-63B guidance for minimum floor.
export function estimateEntropy(passphrase: string): number {
  if (!passphrase) return 0;
  const len = passphrase.length;

  // ── 1. Character pool estimation ──
  let poolSize = 0;
  if (/[a-z]/.test(passphrase)) poolSize += 26;
  if (/[A-Z]/.test(passphrase)) poolSize += 26;
  if (/[0-9]/.test(passphrase)) poolSize += 10;
  if (/[^a-zA-Z0-9]/.test(passphrase)) poolSize += 32;
  if (/[^\x00-\x7F]/.test(passphrase)) poolSize += 100;
  if (poolSize === 0) return 0;

  let entropy = len * Math.log2(poolSize);

  // ── 2. Pattern penalties ──
  // 2a. Sequential characters (abc, 123, etc.)
  const seqPenalty = detectSequential(passphrase);

  // 2b. Repeating characters (aaa, 1111, etc.)
  const repeatPenalty = detectRepeats(passphrase);

  // 2c. Keyboard patterns (qwerty, asdf)
  const kbPenalty = detectKeyboard(passphrase);

  // 2d. Short length penalty
  const shortPenalty = len < 12 ? 0.5 : 1.0;

  // Apply the strongest penalty
  entropy *= Math.min(seqPenalty, repeatPenalty, kbPenalty, shortPenalty);

  // ── 3. NIST SP 800-63B floor ──
  const floor = len >= 12 ? 20.0 : len >= 8 ? 14.0 : 8.0;
  entropy = Math.max(entropy, floor);
  entropy = Math.min(entropy, 128.0);

  return entropy;
}

function detectSequential(s: string): number {
  let runs = 0;
  let longest = 0;
  let current = 1;

  // Ascending
  for (let i = 1; i < s.length; i++) {
    if (s.charCodeAt(i) - s.charCodeAt(i - 1) === 1) {
      current++;
    } else {
      if (current >= 3) { runs++; longest = Math.max(longest, current); }
      current = 1;
    }
  }
  if (current >= 3) { runs++; longest = Math.max(longest, current); }

  // Descending
  current = 1;
  for (let i = 1; i < s.length; i++) {
    if (s.charCodeAt(i - 1) - s.charCodeAt(i) === 1) {
      current++;
    } else {
      if (current >= 3) { runs++; longest = Math.max(longest, current); }
      current = 1;
    }
  }
  if (current >= 3) { runs++; longest = Math.max(longest, current); }

  if (runs === 0) return 1.0;
  const deduction = runs * 0.15 + Math.max(longest, 3) * 0.05;
  return Math.max(1.0 - deduction, 0.3);
}

function detectRepeats(s: string): number {
  let repeats = 0;
  let current = 1;
  for (let i = 1; i < s.length; i++) {
    if (s[i] === s[i - 1]) { current++; }
    else { if (current >= 3) repeats++; current = 1; }
  }
  if (current >= 3) repeats++;
  if (repeats === 0) return 1.0;
  return Math.max(1.0 - repeats * 0.25, 0.2);
}

function detectKeyboard(s: string): number {
  const lower = s.toLowerCase();
  const rows = ["qwertyuiop", "asdfghjkl", "zxcvbnm", "0123456789"];
  const charCount = [...lower].length; // Handle astral Unicode (surrogate pairs)
  let matched = 0;

  for (const row of rows) {
    let i = 0;
    while (i + 2 < charCount) {
      const chunk = [...lower].slice(i, i + 3).join("");
      if (!chunk) { i++; continue; }
      if (row.includes(chunk)) {
        matched += chunk.length;
        i += chunk.length;
        continue;
      }
      const rev = [...chunk].reverse().join("");
      if (row.includes(rev)) {
        matched += chunk.length;
        i += chunk.length;
        continue;
      }
      i++;
    }
  }

  if (matched === 0) return 1.0;
  const ratio = matched / s.length;
  return Math.max(1.0 - ratio * 0.5, 0.3);
}

/// Deterministic HSL color derived from a string (used for avatar gradients).
export function hashToColor(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) hash = str.charCodeAt(i) + ((hash << 5) - hash);
  return `hsl(${Math.abs(hash) % 360}, 55%, 48%)`;
}

/// Relative-time formatter for unix-seconds timestamps ("now", "5m ago", ...).
export function formatTime(ts: number): string {
  const d = Math.floor(Date.now() / 1000) - ts;
  if (d < 60) return "now";
  if (d < 3600) return `${Math.floor(d / 60)}m ago`;
  if (d < 86400) return `${Math.floor(d / 3600)}h ago`;
  if (d < 604800) return `${Math.floor(d / 86400)}d ago`;
  return new Date(ts * 1000).toLocaleDateString();
}

/**
 * Minimum STUN servers that may be configured.
 *
 * Must match `stun::MIN_CONSENSUS_SERVERS` on the Rust side. With one server,
 * "the servers agreed on my public address" is vacuously true — whoever
 * answered *defines* the address this app publishes in invites and plaintext
 * handshakes — so the backend rejects a list shorter than this and so does the
 * remove handler.
 */
export const MIN_STUN_SERVERS = 2;

/// Default STUN servers used when resetting STUN config.
export const DEFAULT_STUN_SERVERS: readonly string[] = [
  "stun.l.google.com:19302",
  "stun1.l.google.com:19302",
  "stun.cloudflare.com:3478",
  "stun.nextcloud.com:3478",
];

/**
 * Extract a human-readable message from an unknown thrown value.
 *
 * Tauri command rejections arrive as an `AppError` — `{ code, message }`, the
 * serialised form of `src-tauri/src/error.rs`. They used to arrive as a bare
 * string, the `Err(String)` side of `Result<T, String>`, and in-process throws
 * are `Error` instances. All three are handled, so this keeps working across
 * the transition and for any command that still rejects with a string.
 *
 * A `catch (e: any)` was used to read `e.message` off all of them — which is
 * exactly the unsound access this helper replaces, and it silently yielded
 * `undefined` for the string case.
 *
 * Returning a non-empty string means the caller can always render something,
 * rather than showing "undefined" to a user.
 *
 * This is the *display* path and deliberately does not validate. For the
 * `{code, message}` shape where a caller wants to branch on the code, use
 * `asAppError()` from `./events` — the `message` check here is intentionally
 * loose so a slightly-off payload still shows the user something instead of
 * falling back to a generic string.
 */
export function errorMessage(e: unknown, fallback = "Unknown error"): string {
  // An empty string is a *possible* rejection value, so `typeof e === "string"`
  // cannot return it unconditionally — doing so would hand the caller `""` and
  // render a blank toast.
  if (typeof e === "string") return e || fallback;
  if (e instanceof Error) return e.message || fallback;
  if (e && typeof e === "object" && "message" in e) {
    // `AppError` from the Rust side. Reading `.message` — not `String(e)`,
    // which is `"[object Object]"` for every command failure since the error
    // taxonomy landed.
    const m = (e as { message: unknown }).message;
    if (typeof m === "string" && m) return m;
  }
  return fallback;
}

/**
 * Copy text to the clipboard, reporting whether it actually worked.
 *
 * `navigator.clipboard.writeText` returns a promise that rejects in a Tauri
 * webview routinely — an unfocused document, a denied permission, or a
 * non-secure context. Fire-and-forget was the bug CLAUDE.md records as fixed in
 * one place and missed in four: the ✓ confirmation (and the clipboard
 * auto-clear timer) was shown unconditionally, so a one-time invite that never
 * left the app looked identical to one that did, and the rejection became an
 * unhandled promise rejection.
 *
 * The caller must gate every success affordance on the return value. This
 * function deliberately does not toast: whether a failed copy warrants a toast
 * depends on what was being copied.
 */
export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/**
 * The standing notice shown when the storage cap permanently destroyed history.
 *
 * Extracted from the `m2m://storage-evicted` listener so the text is testable
 * without mounting the whole app, and so its content cannot drift silently: the
 * three things this must always say are the count, that it cannot be recovered
 * (including from a backup taken beforehand), and that raising the cap is how to
 * stop it recurring. A user who reads "storage limit reached" and nothing else
 * has learned only that something was removed from their machine.
 *
 * Returns `null` when nothing was actually destroyed, so the caller does not
 * display a notice about a loss that did not happen.
 */
export function evictionNoticeText(p: {
  messages_evicted: number;
  group_messages_evicted: number;
  bytes_freed: number;
  overrode_retention: string[];
}): string | null {
  const total = p.messages_evicted + p.group_messages_evicted;
  if (total === 0) return null;
  const mb = p.bytes_freed / (1024 * 1024);
  const freed = mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${Math.round(mb)} MB`;
  const override = p.overrode_retention.length
    ? ` This overrode the retention policy on ${p.overrode_retention.length} conversation(s).`
    : "";
  return (
    `Storage limit reached — ${total} old message(s) were permanently deleted ` +
    `and ${freed} freed. Deleted messages cannot be recovered, including ` +
    `from backups taken beforehand.${override} Raise the cap in Settings to ` +
    `keep more history.`
  );
}
