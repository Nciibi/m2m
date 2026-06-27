import { useState, useCallback } from "react";
import type { ToastData } from "../components/ui/Toast";

let toastCounter = 0;

/**
 * Hook providing toast state management.
 * Can be used from any component.
 */
export function useToast() {
  const [toasts, setToasts] = useState<ToastData[]>([]);

  const addToast = useCallback(
    (message: string, type: ToastData["type"] = "info", duration: number = 4000) => {
      const id = `toast-${++toastCounter}`;
      setToasts((prev) => [...prev, { id, message, type, duration }]);
      if (duration > 0) {
        setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), duration);
      }
    },
    []
  );

  const removeToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  return { toasts, addToast, removeToast };
}
