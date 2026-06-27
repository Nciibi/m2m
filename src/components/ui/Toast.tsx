import { type CSSProperties } from "react";

interface ToastData {
  id: string;
  message: string;
  type: "success" | "error" | "warning" | "info";
  duration?: number;
}

interface ToastContainerProps {
  toasts: ToastData[];
  onRemove: (id: string) => void;
}

const iconMap: Record<string, string> = {
  success: "✅",
  error: "❌",
  warning: "⚠️",
  info: "ℹ️",
};

const variantStyles: Record<string, CSSProperties> = {
  success: {
    background: "rgba(16, 185, 129, 0.15)",
    border: "1px solid rgba(52, 211, 153, 0.2)",
    color: "#a7f3d0",
  },
  error: {
    background: "rgba(239, 68, 68, 0.15)",
    border: "1px solid rgba(248, 113, 113, 0.2)",
    color: "#fca5a5",
  },
  warning: {
    background: "rgba(245, 158, 11, 0.15)",
    border: "1px solid rgba(251, 191, 36, 0.2)",
    color: "#fde68a",
  },
  info: {
    background: "rgba(99, 102, 241, 0.15)",
    border: "1px solid rgba(129, 140, 248, 0.2)",
    color: "#c7d2fe",
  },
};

const progressColors: Record<string, string> = {
  success: "var(--color-success)",
  error: "var(--color-danger)",
  warning: "var(--color-warning)",
  info: "var(--color-accent)",
};

/**
 * Renders a floating stack of toast notifications at the bottom-right.
 * Each toast has an auto-dismiss progress bar.
 */
export function ToastContainer({ toasts, onRemove }: ToastContainerProps) {
  if (toasts.length === 0) return null;

  return (
    <div
      className="toast-container"
      style={{
        position: "fixed",
        bottom: 24,
        right: 24,
        zIndex: "var(--z-toast)",
        display: "flex",
        flexDirection: "column",
        gap: 8,
        maxWidth: 400,
        pointerEvents: "none",
      }}
    >
      {toasts.map((t) => (
        <div
          key={t.id}
          className={`toast toast-${t.type}`}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            padding: "14px 16px",
            borderRadius: "var(--radius-md)",
            fontSize: "var(--text-md)",
            lineHeight: 1.4,
            boxShadow: "0 8px 32px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.05)",
            pointerEvents: "auto",
            animation: "toastSlideIn 0.3s cubic-bezier(0.16,1,0.3,1) forwards",
            cursor: "pointer",
            position: "relative",
            overflow: "hidden",
            ...variantStyles[t.type],
          }}
          onClick={() => onRemove(t.id)}
          role="alert"
          aria-live="assertive"
        >
          {/* Progress bar */}
          <span
            style={{
              position: "absolute",
              bottom: 0,
              left: 0,
              height: 3,
              background: progressColors[t.type],
              animation: `toastProgress ${(t.duration || 4000)}ms linear forwards`,
              opacity: 0.5,
            }}
          />
          <span className="toast-icon" style={{ fontSize: "1rem", flexShrink: 0 }}>
            {iconMap[t.type]}
          </span>
          <span className="toast-message" style={{ flex: 1 }}>
            {t.message}
          </span>
          <button
            onClick={(e) => {
              e.stopPropagation();
              onRemove(t.id);
            }}
            aria-label="Dismiss notification"
            style={{
              background: "none",
              border: "none",
              color: "rgba(255,255,255,0.4)",
              padding: 0,
              minWidth: "auto",
              cursor: "pointer",
              fontSize: "1rem",
              lineHeight: 1,
              flexShrink: 0,
              fontFamily: "inherit",
            }}
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}

export type { ToastData };
