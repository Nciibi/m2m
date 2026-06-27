import { type InputHTMLAttributes, type ReactNode, useRef } from "react";

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
  /** Wrapper style for the container */
  wrapperStyle?: React.CSSProperties;
}

/**
 * Styled input with icon adornment, error state, and clearable support.
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

  const containerStyle: React.CSSProperties = {
    display: "flex",
    flexDirection: "column",
    gap: 4,
    width: "100%",
    ...wrapperStyle,
  };

  const inputWrapperStyle: React.CSSProperties = {
    display: "flex",
    alignItems: "center",
    gap: 8,
    background: "var(--color-bg-input)",
    border: `1px solid ${error ? "var(--color-danger)" : "var(--color-border-default)"}`,
    borderRadius: "var(--radius-md)",
    padding: compact ? "6px 12px" : "10px 16px",
    transition: "var(--transition-fast)",
    position: "relative",
  };

  const inputStyle: React.CSSProperties = {
    flex: 1,
    background: "none",
    border: "none",
    outline: "none",
    color: "var(--color-text-primary)",
    fontSize: compact ? "var(--text-base)" : "var(--text-md)",
    fontFamily: mono ? "var(--font-mono)" : "var(--font-sans)",
    padding: 0,
    width: "100%",
  };

  const handleFocus = (e: React.FocusEvent<HTMLInputElement>) => {
    const wrapper = e.currentTarget.parentElement;
    if (wrapper) {
      wrapper.style.borderColor = "var(--color-border-active)";
      wrapper.style.background = "var(--color-bg-input-focus)";
      wrapper.style.boxShadow = "0 0 0 3px var(--color-accent-glow)";
    }
  };

  const handleBlur = (e: React.FocusEvent<HTMLInputElement>) => {
    const wrapper = e.currentTarget.parentElement;
    if (wrapper) {
      wrapper.style.borderColor = error
        ? "var(--color-danger)"
        : "var(--color-border-default)";
      wrapper.style.background = "var(--color-bg-input)";
      wrapper.style.boxShadow = "none";
    }
  };

  return (
    <div style={containerStyle}>
      <div
        style={inputWrapperStyle}
        className="input-wrapper"
      >
        {icon && (
          <span style={{ color: "var(--color-text-muted)", fontSize: "1rem", lineHeight: 1, flexShrink: 0 }}>
            {icon}
          </span>
        )}
        <input
          ref={inputRef}
          style={inputStyle}
          value={value}
          onChange={onChange}
          onFocus={handleFocus}
          onBlur={handleBlur}
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
              padding: 0,
              fontSize: "1rem",
              lineHeight: 1,
              flexShrink: 0,
              fontFamily: "inherit",
            }}
            aria-label="Clear input"
          >
            ✕
          </button>
        )}
      </div>
      {error && (
        <span
          className="input-error"
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
