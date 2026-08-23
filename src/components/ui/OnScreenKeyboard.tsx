import { useCallback, useEffect, useMemo, useState } from "react";
import "./OnScreenKeyboard.css";

/**
 * On-screen keyboard for high-value secret entry (vault passphrase).
 *
 * Threat model (security roadmap §2): defeats HOOK-BASED keyloggers that
 * intercept physical keyboard events — keystrokes never pass through the
 * OS input stack for this field. It does NOT defeat screen recorders
 * watching the mouse, or kernel-level loggers; pair with focus-blur and
 * capture protection.
 *
 * Security properties:
 * - Layout is Fisher–Yates shuffled on every open AND after every insert,
 *   so click positions leak no frequency information about the secret.
 * - Buttons use `onMouseDown` + `preventDefault` so the target input never
 *   loses focus (physical typing keeps working alongside clicks).
 * - Characters are NOT placed in DOM titles/tooltips.
 */

interface Props {
  /** Called for each character the user taps. */
  onInsert: (ch: string) => void;
  /** Called when the backspace key is tapped. */
  onBackspace: () => void;
  /** Controlled visibility. */
  open: boolean;
  onClose: () => void;
}

/** Rows of candidate glyphs; order within each row is shuffled per render. */
const KEY_GROUPS: string[][] = [
  // Lowercase letters (uppercase via Shift toggle)
  "abcdefghijklmnopqrstuvwxyz".split(""),
  // Digits
  "0123456789".split(""),
  // Common passphrase symbols
  ["!", "@", "#", "$", "%", "^", "&", "*", "(", ")", "-", "_", "=", "+"],
  [".", ",", ";", ":", "'", '"', "?", "/", "\\", "[", "]", "{", "}"],
];

function shuffle<T>(arr: T[]): T[] {
  const a = [...arr];
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

export default function OnScreenKeyboard({ onInsert, onBackspace, open, onClose }: Props) {
  const [shifted, setShifted] = useState(false);
  // Bump to force a re-shuffle after every insertion.
  const [shuffleEpoch, setShuffleEpoch] = useState(0);

  // Fresh random layout every time the keyboard opens.
  useEffect(() => {
    if (open) setShuffleEpoch((e) => e + 1);
  }, [open]);

  const rows = useMemo(() => {
    void shuffleEpoch;
    return KEY_GROUPS.map((group) => {
      const letters = group.map((k) => (shifted && k.match(/[a-z]/) ? k.toUpperCase() : k));
      return shuffle(letters);
    });
  }, [shifted, shuffleEpoch]);

  const press = useCallback(
    (fn: () => void) => (e: React.MouseEvent | React.TouchEvent) => {
      e.preventDefault(); // keep focus in the target input
      fn();
    },
    [],
  );

  if (!open) return null;

  return (
    <div className="osk" role="group" aria-label="On-screen keyboard">
      <div className="osk__header">
        <span className="osk__hint">Tap keys — layout reshuffles after each press</span>
        <button type="button" className="osk__close" onClick={onClose} aria-label="Close on-screen keyboard">
          ✕
        </button>
      </div>

      {rows.map((row, ri) => (
        <div className="osk__row" key={ri}>
          {ri === 0 && (
            <button
              type="button"
              className={`osk__key osk__key--modifier ${shifted ? "osk__key--active" : ""}`}
              onMouseDown={press(() => setShifted((s) => !s))}
              aria-label="Toggle uppercase"
              aria-pressed={shifted}
            >
              ⇧
            </button>
          )}
          {row.map((key) => (
            <button
              type="button"
              className="osk__key"
              onMouseDown={press(() => {
                onInsert(key);
                setShuffleEpoch((e) => e + 1);
              })}
              key={`${ri}-${key}-${shuffleEpoch}`}
              aria-label={`Insert character ${key}`}
            >
              {key}
            </button>
          ))}
        </div>
      ))}

      <div className="osk__row">
        <button
          type="button"
          className="osk__key osk__key--wide"
          onMouseDown={press(onBackspace)}
          aria-label="Backspace"
        >
          ⌫ Backspace
        </button>
      </div>
    </div>
  );
}
