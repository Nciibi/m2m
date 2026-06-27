import { type SelectHTMLAttributes } from "react";

interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps extends Omit<SelectHTMLAttributes<HTMLSelectElement>, "children"> {
  options: SelectOption[];
  placeholder?: string;
  error?: string;
  compact?: boolean;
  fullWidth?: boolean;
}

export default function Select({
  options,
  placeholder,
  error,
  compact = false,
  fullWidth = true,
  value,
  style,
  className = "",
  ...rest
}: SelectProps & { className?: string }) {
  return (
    <div className={`select-wrap ${className}`} style={{ width: fullWidth ? "100%" : undefined, ...style }}>
      <select
        className={compact ? "select--compact" : ""}
        value={value}
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
      {error && <span className="input__error">{error}</span>}
    </div>
  );
}
