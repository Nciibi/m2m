import { type CSSProperties, type SelectHTMLAttributes } from "react";

interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps extends Omit<SelectHTMLAttributes<HTMLSelectElement>, "children"> {
  options: SelectOption[];
  /** Placeholder when no value is selected (rendered as a disabled option) */
  placeholder?: string;
  /** Error message */
  error?: string;
  /** Compact size */
  compact?: boolean;
  /** Full-width (default) */
  fullWidth?: boolean;
}

/**
 * Styled select dropdown with custom chevron arrow.
 * Follows the same visual language as Input.
 */
export default function Select({
  options,
  placeholder,
  error,
  compact = false,
  fullWidth = true,
  value,
  style,
  ...rest
}: SelectProps) {
  const containerStyle: CSSProperties = {
    display: "flex",
    flexDirection: "column",
    gap: 4,
    width: fullWidth ? "100%" : undefined,
  };

  const selectStyle: CSSProperties = {
    background: "var(--color-bg-input)",
    border: `1px solid ${error ? "var(--color-danger)" : "var(--color-border-default)"}`,
    color: "var(--color-text-primary)",
    padding: compact ? "8px 14px" : "10px 14px",
    borderRadius: "var(--radius-md)",
    fontSize: compact ? "var(--text-base)" : "var(--text-md)",
    fontFamily: "var(--font-sans)",
    outline: "none",
    cursor: "pointer",
    transition: "var(--transition-fast)",
    width: "100%",
    appearance: "none",
    paddingRight: 40,
    backgroundImage: `url("data:image/svg+xml;charset=UTF-8,%3csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='rgba(255,255,255,0.5)' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3e%3cpolyline points='6 9 12 15 18 9'%3e%3c/polyline%3e%3c/svg%3e")`,
    backgroundRepeat: "no-repeat",
    backgroundPosition: "right 12px center",
    backgroundSize: 16,
    ...style,
  };

  return (
    <div style={containerStyle}>
      <select
        style={selectStyle}
        value={value}
        onFocus={(e) => {
          e.currentTarget.style.borderColor = "var(--color-border-active)";
          e.currentTarget.style.background = "var(--color-bg-input-focus)";
          e.currentTarget.style.boxShadow = "0 0 0 3px var(--color-accent-glow)";
        }}
        onBlur={(e) => {
          e.currentTarget.style.borderColor = error
            ? "var(--color-danger)"
            : "var(--color-border-default)";
          e.currentTarget.style.background = "var(--color-bg-input)";
          e.currentTarget.style.boxShadow = "none";
        }}
        {...rest}
      >
        {placeholder && (
          <option value="" disabled>
            {placeholder}
          </option>
        )}
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
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
