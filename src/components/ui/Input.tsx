import { type InputHTMLAttributes, type ReactNode, useRef } from "react";
import { CloseIcon, SearchIcon } from "./Icons";

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  /** Icon displayed at the left of the input */
  icon?: ReactNode;
  /** Error message shown below */
  error?: string;
  /** If true, shows a clear (✕) button when value is non-empty */
  clearable?: boolean;
  /** Callback when the clear button is clicked */
  onClear?: () => void;
  /** Compact size */
  compact?: boolean;
  /** Monospace font (for fingerprints, invite codes) */
  mono?: boolean;
  /** Wrapper style */
  wrapperStyle?: React.CSSProperties;
}

/**
 * Premium input with focus glow, icon support, clearable, error state.
 * No browser default focus rings — all custom.
 */
export default function Input({
  icon,
  error,
  clearable,
  onClear,
  compact = false,
  mono = false,
  wrapperStyle,
  value,
  onChange,
  style,
  ...rest
}: InputProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  const hasValue =
    value !== undefined && value !== null && String(value).length > 0;

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 4,
        width: "100%",
        ...wrapperStyle,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          background: error
            ? "var(--color-danger-bg)"
            : "var(--color-bg-input)",
          border: `1px solid ${
            error ? "var(--color-danger)" : "var(--color-border-default)"
          }`,
          borderRadius: "var(--radius-md)",
          padding: compact ? "6px 12px" : "10px 16px",
          transition: "border-color 150ms ease, background 150ms ease, box-shadow 150ms ease",
          position: "relative",
        }}
        className="input-wrapper"
        onMouseDown={(e) => {
          // Click on wrapper focuses the input
          if (e.target === e.currentTarget) {
            inputRef.current?.focus();
          }
        }}
      >
        {icon && (
          <span
            style={{
              color: "var(--color-text-muted)",
              fontSize: 0,
              lineHeight: 1,
              flexShrink: 0,
              display: "flex",
            }}
          >
            {icon}
          </span>
        )}
        <input
          ref={inputRef}
          style={{
            flex: 1,
            background: "none",
            border: "none",
            outline: "none",
            boxShadow: "none",
            color: "var(--color-text-primary)",
            fontSize: compact ? "var(--text-base)" : "var(--text-md)",
            fontFamily: mono ? "var(--font-mono)" : "var(--font-sans)",
            padding: 0,
            width: "100%",
            WebkitAppearance: "none",
          }}
          value={value}
          onChange={onChange}
          onFocus={(e) => {
            const wrapper = e.currentTarget.parentElement;
            if (wrapper) {
              wrapper.style.borderColor = "var(--color-border-active)";
              wrapper.style.background = "var(--color-bg-input-focus)";
              wrapper.style.boxShadow = "0 0 0 3px var(--color-accent-glow)";
            }
          }}
          onBlur={(e) => {
            const wrapper = e.currentTarget.parentElement;
            if (wrapper) {
              wrapper.style.borderColor = error
                ? "var(--color-danger)"
                : "var(--color-border-default)";
              wrapper.style.background = error
                ? "var(--color-danger-bg)"
                : "var(--color-bg-input)";
              wrapper.style.boxShadow = "none";
            }
          }}
          {...rest}
        />
        {clearable && hasValue && onClear && (
          <button
            type="button"
            onClick={(e) => {
              e.preventDefault();
              onClear();
              inputRef.current?.focus();
            }}
            style={{
              background: "none",
              border: "none",
              color: "var(--color-text-muted)",
              cursor: "pointer",
              padding: 4,
              lineHeight: 1,
              flexShrink: 0,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontFamily: "inherit",
              borderRadius: "var(--radius-xs)",
              transition: "color 150ms ease",
            }}
            aria-label="Clear input"
          >
            <CloseIcon size={16} />
          </button>
        )}
      </div>
      {error && (
        <span
          style={{
            fontSize: "var(--text-sm)",
            color: "var(--color-danger)",
            paddingLeft: 4,
          }}
        >
          {error}
        </span>
      )}
    </div>
  );
}
