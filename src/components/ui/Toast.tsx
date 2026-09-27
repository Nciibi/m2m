import { CheckIcon, CloseIcon, AlertTriangleIcon, InfoIcon } from "./Icons";

export interface ToastData {
  id: string;
  message: string;
  type: "success" | "error" | "warning" | "info";
  duration?: number;
}

interface ToastContainerProps {
  toasts: ToastData[];
  onRemove: (id: string) => void;
}

const iconMap: Record<string, React.ReactNode> = {
  success: <CheckIcon size={16} color="var(--color-success)" />,
  error: <CloseIcon size={16} color="var(--color-danger)" />,
  warning: <AlertTriangleIcon size={16} color="var(--color-warning)" />,
  info: <InfoIcon size={16} color="var(--color-accent-bright)" />,
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
          // Errors interrupt; informational toasts wait for a pause. Marking
          // every toast `role="alert"` + `aria-live="assertive"` meant routine
          // "Copied" / "Theme: dark" messages cut across whatever a screen
          // reader was in the middle of saying.
          role={t.type === "error" ? "alert" : "status"}
          aria-live={t.type === "error" ? "assertive" : "polite"}
        >
          <span
            className="toast__progress"
            style={{
              background: progressColors[t.type],
              animationName: "toastProgress",
              animationDuration: `${t.duration || 4000}ms`,
              animationTimingFunction: "linear",
              animationFillMode: "forwards",
            }}
          />
          <span className="toast__icon">{iconMap[t.type]}</span>
          <span className="toast__msg">{t.message}</span>
          <button
            className="toast__dismiss"
            onClick={(e) => { e.stopPropagation(); onRemove(t.id); }}
            aria-label="Dismiss notification"
          >
            <CloseIcon size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
