import { type InputHTMLAttributes, type ReactNode, useRef } from "react";
import { CloseIcon } from "./Icons";

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  icon?: ReactNode;
  error?: string;
  clearable?: boolean;
  onClear?: () => void;
  compact?: boolean;
  mono?: boolean;
}

export default function Input({
  icon,
  error,
  clearable,
  onClear,
  compact = false,
  mono = false,
  value,
  onChange,
  className = "",
  onFocus: callerOnFocus,
  onBlur: callerOnBlur,
  ...rest
}: InputProps & { className?: string }) {
  const inputRef = useRef<HTMLInputElement>(null);
  const hasValue =
    value !== undefined && value !== null && String(value).length > 0;

  // Composed, not replaced.
  //
  // `{...rest}` used to be spread *after* `onFocus`/`onBlur`, so a caller
  // passing its own silently replaced the internal handlers and the
  // `input-wrap--focused` / `input-wrap--error` toggling stopped working.
  // `VaultView` passes `onFocus` on *both* passphrase fields to drive the
  // on-screen keyboard, so the two fields that most need a visible focus ring
  // never had one — and no test asserted on the class, so nothing caught it.
  //
  // Simply moving `{...rest}` earlier is not the fix: that would stop the
  // caller's handler firing at all, which breaks the on-screen keyboard the same
  // passphrase fields depend on. Both have to run.
  const handleFocus = (e: React.FocusEvent<HTMLInputElement>) => {
    const wrap = e.currentTarget.closest(".input-wrap") as HTMLElement;
    if (wrap) {
      wrap.classList.add("input-wrap--focused");
      if (error) wrap.classList.remove("input-wrap--error");
    }
    callerOnFocus?.(e);
  };

  const handleBlur = (e: React.FocusEvent<HTMLInputElement>) => {
    const wrap = e.currentTarget.closest(".input-wrap") as HTMLElement;
    if (wrap) {
      wrap.classList.remove("input-wrap--focused");
      if (error) wrap.classList.add("input-wrap--error");
    }
    callerOnBlur?.(e);
  };

  return (
    <div className={`input-group ${className}`}>
      <div
        className={`input-wrap ${compact ? "input-wrap--compact" : ""} ${error ? "input-wrap--error" : ""}`}
      >
        {icon && <span className="input__icon">{icon}</span>}
        <input
          ref={inputRef}
          className={mono ? "input--mono" : ""}
          value={value}
          onChange={onChange}
          // `rest` is spread FIRST, so the internal handlers below always win.
          //
          // It used to be spread last, which meant any caller passing its own
          // `onFocus`/`onBlur` silently replaced `handleFocus`/`handleBlur` and
          // the `input-wrap--focused` / `input-wrap--error` class toggling
          // stopped working entirely. `VaultView` passes `onFocus` on *both*
          // passphrase fields to drive the on-screen keyboard, so the two fields
          // that most need a visible focus ring never had one — and no test
          // asserted on the class, so nothing caught it.
          {...rest}
          onFocus={handleFocus}
          onBlur={handleBlur}
        />
        {clearable && hasValue && onClear && (
          <button
            type="button"
            className="input__clear"
            onClick={(e) => {
              e.preventDefault();
              onClear();
              inputRef.current?.focus();
            }}
            aria-label="Clear input"
          >
            <CloseIcon size={16} />
          </button>
        )}
      </div>
      {error && <span className="input__error">{error}</span>}
    </div>
  );
}
