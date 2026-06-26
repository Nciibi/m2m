import { useState, useCallback } from "react";
import type { Toast } from "./types";

let toastCounter = 0;

export function ToastContainer({
  toasts,
  onRemove,
}: {
  toasts: Toast[];
  onRemove: (id: string) => void;
}) {
  if (toasts.length === 0) return null;
  return (
    <div className="toast-container" id="toast-container">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={`toast toast-${t.type}`}
          onClick={() => onRemove(t.id)}
        >
          <span className="toast-icon">
            {t.type === "success"
              ? "✅"
              : t.type === "error"
                ? "❌"
                : t.type === "warning"
                  ? "⚠️"
                  : "ℹ️"}
          </span>
          <span className="toast-message">{t.message}</span>
          <button
            className="toast-dismiss"
            onClick={(e) => {
              e.stopPropagation();
              onRemove(t.id);
            }}
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}

/** Convenience hook for toast state management. */
export function useToast() {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const addToast = useCallback(
    (
      message: string,
      type: Toast["type"] = "info",
      duration: number = 4000
    ) => {
      const id = `toast-${++toastCounter}`;
      setToasts((prev) => [...prev, { id, message, type, duration }]);
      if (duration > 0) {
        setTimeout(
          () => setToasts((prev) => prev.filter((t) => t.id !== id)),
          duration
        );
      }
    },
    []
  );

  const removeToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  return { toasts, addToast, removeToast };
}
