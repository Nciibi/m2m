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

const progressColors: Record<string, string> = {
  success: "var(--color-success)",
  error: "var(--color-danger)",
  warning: "var(--color-warning)",
  info: "var(--color-accent)",
};

export function ToastContainer({ toasts, onRemove }: ToastContainerProps) {
  if (toasts.length === 0) return null;

  return (
    <div className="toast-container">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={`toast toast--${t.type}`}
          onClick={() => onRemove(t.id)}
          role="alert"
          aria-live="assertive"
        >
          <span
            className="toast__progress"
            style={{
              background: progressColors[t.type],
              animation: `toastProgress ${(t.duration || 4000)}ms linear forwards`,
            }}
          />
          <span className="toast__icon">{iconMap[t.type]}</span>
          <span className="toast__msg">{t.message}</span>
          <button
            className="toast__dismiss"
            onClick={(e) => { e.stopPropagation(); onRemove(t.id); }}
            aria-label="Dismiss notification"
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}

export type { ToastData };
