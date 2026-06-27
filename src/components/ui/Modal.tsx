import { type ReactNode, useEffect, useRef } from "react";

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  /** Optional footer with action buttons */
  footer?: ReactNode;
  /** Max width override */
  maxWidth?: number;
}

/**
 * Accessible dialog modal with focus trap, ESC to close, and backdrop click to close.
 * Renders inside a portal at z-index 9999.
 */
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

  // Focus trap + ESC handler
  useEffect(() => {
    if (!open) return;

    previousFocus.current = document.activeElement as HTMLElement;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
        return;
      }

      // Focus trap: Tab/Shift+Tab cycle within the modal
      if (e.key === "Tab" && dialogRef.current) {
        const focusable = dialogRef.current.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (focusable.length === 0) return;

        const first = focusable[0];
        const last = focusable[focusable.length - 1];

        if (e.shiftKey) {
          if (document.activeElement === first) {
            e.preventDefault();
            last.focus();
          }
        } else {
          if (document.activeElement === last) {
            e.preventDefault();
            first.focus();
          }
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    // Auto-focus first focusable element
    requestAnimationFrame(() => {
      const first = dialogRef.current?.querySelector<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      first?.focus();
    });

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      previousFocus.current?.focus();
    };
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: "var(--z-modal)",
        background: "var(--color-bg-overlay)",
        backdropFilter: "blur(8px)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        animation: "modalFadeIn 0.2s ease-out",
      }}
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-label={title}
    >
      <div
        ref={dialogRef}
        onClick={(e) => e.stopPropagation()}
        role="document"
        style={{
          background: "var(--color-bg-surface)",
          border: "1px solid rgba(255,255,255,0.08)",
          borderRadius: "var(--radius-2xl)",
          padding: "var(--space-2xl)",
          maxWidth,
          width: "90%",
          maxHeight: "85vh",
          overflowY: "auto",
          boxShadow: "var(--shadow-xl)",
          animation: "modalZoomIn 0.3s cubic-bezier(0.16,1,0.3,1)",
          display: "flex",
          flexDirection: "column",
          gap: "var(--space-lg)",
        }}
      >
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <h2
            style={{
              margin: 0,
              fontSize: "var(--text-xl)",
              fontWeight: 600,
              display: "flex",
              alignItems: "center",
              gap: 8,
            }}
          >
            {title}
          </h2>
          <button
            onClick={onClose}
            aria-label="Close dialog"
            style={{
              background: "none",
              border: "1px solid var(--color-border-default)",
              color: "var(--color-text-secondary)",
              padding: "4px 12px",
              borderRadius: 8,
              cursor: "pointer",
              fontSize: "1.1rem",
              fontFamily: "inherit",
            }}
          >
            ✕
          </button>
        </div>

        {/* Body */}
        <div style={{ fontSize: "var(--text-md)", color: "var(--color-text-secondary)", lineHeight: 1.6 }}>
          {children}
        </div>

        {/* Footer */}
        {footer && (
          <div style={{ display: "flex", gap: "var(--space-sm)", justifyContent: "flex-end", marginTop: 8 }}>
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}
