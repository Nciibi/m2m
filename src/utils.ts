/// Estimate passphrase entropy in bits using character-pool + pattern detection.
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
  let penalty = 1.0;

  // 2a. Sequential characters (abc, 123, etc.)
  const seqPenalty = detectSequential(passphrase);

  // 2b. Repeating characters (aaa, 1111, etc.)
  const repeatPenalty = detectRepeats(passphrase);

  // 2c. Keyboard patterns (qwerty, asdf)
  const kbPenalty = detectKeyboard(passphrase);

  // 2d. Short length penalty
  const shortPenalty = len < 12 ? 0.5 : 1.0;

  // Apply the strongest penalty
  penalty = Math.min(seqPenalty, repeatPenalty, kbPenalty, shortPenalty);
  entropy *= penalty;

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
