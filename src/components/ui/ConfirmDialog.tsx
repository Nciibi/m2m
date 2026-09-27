import { useState } from "react";
import Modal from "./Modal";
import Button from "./Button";
import { useT } from "../../i18n/I18nContext";

/**
 * A styled, localisable confirmation dialog.
 *
 * Replaces `window.confirm` for irreversible actions. See
 * `src/i18n/catalog.ts` for why native dialogs are unacceptable for the
 * panic-wipe and duress flows specifically.
 */
export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel,
  cancelLabel,
  onConfirm,
  onCancel,
  destructive = false,
}: {
  open: boolean;
  title: string;
  body: string;
  confirmLabel: string;
  cancelLabel: string;
  onConfirm: () => void | Promise<void>;
  onCancel: () => void;
  /** Styles the confirm button as destructive and focuses Cancel first. */
  destructive?: boolean;
}) {
  const [busy, setBusy] = useState(false);

  return (
    <Modal
      open={open}
      onClose={busy ? () => {} : onCancel}
      title={title}
      footer={
        <>
          <Button variant="secondary" onClick={onCancel} disabled={busy}>
            {cancelLabel}
          </Button>
          <Button
            variant={destructive ? "danger" : "primary"}
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              try {
                await onConfirm();
              } finally {
                setBusy(false);
              }
            }}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <p>{body}</p>
    </Modal>
  );
}

/**
 * Collects and confirms a duress passphrase.
 *
 * Replaces a `window.prompt` followed by a `window.confirm`. Two things the
 * native version got wrong that matter here:
 *
 *  1. **No length feedback while typing.** The prompt said "min 12 chars" but
 *     only the Rust command enforced it, so a user could type a short value
 *     and only then be told it was rejected — after having been asked to
 *     confirm an irreversible action.
 *  2. **No distinction between the two phases.** Typing a passphrase and
 *     agreeing to its consequences were separate, unstyled popups, which makes
 *     it easy to agree to the second without having read the first.
 *
 * Both are now one dialog with live validation and the consequence stated
 * above the field rather than buried in a prompt body.
 */
export function DuressPassphraseDialog({
  open,
  onClose,
  onSubmit,
}: {
  open: boolean;
  onClose: () => void;
  onSubmit: (passphrase: string) => void | Promise<void>;
}) {
  const t = useT();
  const [value, setValue] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const tooShort = value.length > 0 && value.length < 12;
  const mismatched = confirm.length > 0 && confirm !== value;
  const canSubmit = value.length >= 12 && confirm === value && !busy;

  function reset() {
    // Never leave a passphrase in component state after the dialog closes.
    setValue("");
    setConfirm("");
    setError(null);
    setBusy(false);
  }

  function handleClose() {
    if (busy) return;
    reset();
    onClose();
  }

  return (
    <Modal
      open={open}
      onClose={handleClose}
      title={t("settings.duressSetTitle")}
      footer={
        <>
          <Button variant="secondary" onClick={handleClose} disabled={busy}>
            {t("generic.cancel")}
          </Button>
          <Button
            variant="danger"
            disabled={!canSubmit}
            onClick={async () => {
              setBusy(true);
              setError(null);
              try {
                await onSubmit(value);
                reset();
              } catch (e) {
                // Keep the typed value so the user can correct it, and say
                // what went wrong rather than closing on them.
                setError(String(e));
                setBusy(false);
              }
            }}
          >
            {t("settings.duressConfirmAction")}
          </Button>
        </>
      }
    >
      <p>{t("settings.duressSetBody")}</p>
      <p>
        <strong>{t("settings.duressSetWarning")}</strong>
      </p>

      <div className="field-group">
        <label className="field-label" htmlFor="duress-passphrase">
          {t("settings.duressSetLabel")}
        </label>
        <input
          id="duress-passphrase"
          className="input"
          type="password"
          autoComplete="off"
          value={value}
          placeholder={t("settings.duressSetPlaceholder")}
          onChange={(e) => setValue(e.target.value)}
          aria-describedby="duress-help"
        />
        <span id="duress-help" className="settings-hint">
          {t("vault.minLength")}
        </span>
        {tooShort && (
          <span className="field-error" role="alert">
            {t("vault.minLength")}
          </span>
        )}
      </div>

      <div className="field-group">
        <label className="field-label" htmlFor="duress-passphrase-confirm">
          {t("vault.confirmPassphrase")}
        </label>
        <input
          id="duress-passphrase-confirm"
          className="input"
          type="password"
          autoComplete="off"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
        />
        {mismatched && (
          <span className="field-error" role="alert">
            {t("vault.mismatch")}
          </span>
        )}
      </div>

      {error && (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
    </Modal>
  );
}
