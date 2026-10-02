import { type ReactNode, useEffect, useRef } from "react";
import { CloseIcon } from "./Icons";

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  footer?: ReactNode;
  maxWidth?: number;
}

export default function Modal({
  open,
  onClose,
  title,
  children,
  footer,
  maxWidth = 560,
}: ModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  // Latest `onClose` without making it an effect dependency — see the note on
  // the effect below.
  const onCloseRef = useRef(onClose);
  useEffect(() => { onCloseRef.current = onClose; }, [onClose]);

  useEffect(() => {
    if (!open) return;
    previousFocus.current = document.activeElement as HTMLElement;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") { onCloseRef.current(); return; }
      if (e.key === "Tab" && dialogRef.current) {
        const focusable = dialogRef.current.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (focusable.length === 0) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (e.shiftKey) {
          if (document.activeElement === first) { e.preventDefault(); last.focus(); }
        } else {
          if (document.activeElement === last) { e.preventDefault(); first.focus(); }
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);

    // Focus the first field, but do NOT steal focus if the user has already
    // moved it.
    //
    // This previously ran unconditionally on the next animation frame, which
    // could land *after* the user started typing: keystrokes were silently
    // swallowed from that point on (observed as a 28-character value arriving as
    // "co"). Guarding on `document.activeElement` means a fast typist keeps
    // their caret.
    //
    // A microtask is used rather than `requestAnimationFrame` because the
    // dialog is laid out by the time the effect runs, and the extra frame only
    // widened the window for the race.
    queueMicrotask(() => {
      if (!open) return;
      const active = document.activeElement;
      const alreadyInside = active instanceof HTMLElement && dialogRef.current?.contains(active);
      if (alreadyInside) return;
      const first = dialogRef.current?.querySelector<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      first?.focus();
    });

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      previousFocus.current?.focus();
    };
    // `onClose` is deliberately NOT a dependency.
    //
    // Callers pass an inline arrow (`onClose={() => setShowAdd(false)}`), so
    // its identity changed on every parent render. With `onClose` in the array,
    // typing in a text field re-ran this effect, which overwrote
    // `previousFocus` with the currently-focused element *inside* the dialog
    // and re-fired the `requestAnimationFrame` that focuses the FIRST field —
    // so focus jumped back to the top field on every keystroke and the user
    // physically could not type a nickname.
    //
    // `handleKeyDown` only ever needs a stable way to CALL `onClose`, so a ref
    // gives the correct behaviour without re-running the effect.
  }, [open]);

  if (!open) return null;

  // `role="dialog"` belongs on the panel, not the backdrop.
  //
  // On the overlay it described the full-screen click-catcher as the dialog, and
  // the actual dialog panel was `role="document"` — so assistive tech was told
  // the dialog was the thing behind it. `aria-modal` on the overlay also does
  // not make the rest of the page inert; the Tab trap handles keyboard order,
  // but a screen reader's virtual cursor and background click targets are still
  // live. `inert` on the background is what actually hides them.
  const background = document.querySelector<HTMLElement>("#app-root > *:not(.modal-layer)");

  return (
    <>
      {background &&
        Array.from(document.querySelectorAll<HTMLElement>("#app-root > *:not(.modal-layer)")).map(
          (el) => (
            <div key={el.id || el.className} className="modal-backdrop-blocker" inert />
          ),
        )}
      <div className="modal-layer">
        <div className="modal-overlay" onClick={onClose}>
          <div
            ref={dialogRef}
            className="modal"
            style={{ maxWidth }}
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-modal="true"
            // `aria-labelledby` points at the visible <h2> rather than
            // duplicating the title string in `aria-label`: one source of truth,
            // and no chance of the two drifting apart.
            aria-labelledby="modal-title"
          >
            <div className="modal__header">
              <h2 className="modal__title" id="modal-title">{title}</h2>
              <button type="button" className="modal__close" onClick={onClose} aria-label="Close dialog">
                <CloseIcon size={18} />
              </button>
            </div>
            <div className="modal__body">{children}</div>
            {footer && <div className="modal__footer">{footer}</div>}
          </div>
        </div>
      </div>
    </>
  );
}
