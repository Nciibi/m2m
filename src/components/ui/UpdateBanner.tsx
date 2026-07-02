import { useState, useEffect } from "react";
import { CheckIcon, CloseIcon } from "./Icons";

/**
 * UpdateBanner — checks for updates and shows a non-blocking banner.
 * Requires tauri-plugin-updater to be configured with endpoints.
 * Without endpoints configured, this silently returns null.
 */
export default function UpdateBanner() {
  const [updateAvailable, setUpdateAvailable] = useState(false);
  const [updateVersion, setUpdateVersion] = useState("");
  const [checking, setChecking] = useState(true);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const { check } = await import("@tauri-apps/plugin-updater");
        const update = await check();
        if (!cancelled) {
          if (update?.available) {
            setUpdateAvailable(true);
            setUpdateVersion(update.version || "");
          }
          setChecking(false);
        }
      } catch {
        // Updater not configured — silently ignore
        if (!cancelled) setChecking(false);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  if (checking || !updateAvailable || dismissed) return null;

  return (
    <div className="update-banner">
      <div className="update-banner__content">
        <span className="update-banner__icon">📦</span>
        <span className="update-banner__text">
          Update available: <strong>v{updateVersion}</strong>
        </span>
        <button
          className="btn btn--sm btn--accent"
          onClick={async () => {
            try {
              const { check, install } = await import("@tauri-apps/plugin-updater");
              const update = await check();
              if (update?.available) {
                await update.downloadAndInstall();
              }
            } catch {
              // Install failed — ignore
            }
          }}
        >
          <CheckIcon size={14} /> Update Now
        </button>
        <button
          className="btn btn--icon btn--icon-sm"
          onClick={() => setDismissed(true)}
          aria-label="Dismiss"
        >
          <CloseIcon size={14} />
        </button>
      </div>
    </div>
  );
}
