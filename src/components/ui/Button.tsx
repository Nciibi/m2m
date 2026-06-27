import { type ButtonHTMLAttributes, type ReactNode, useRef } from "react";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "default" | "secondary" | "danger" | "ghost" | "icon";
  loading?: boolean;
  icon?: ReactNode;
  fullWidth?: boolean;
  size?: "sm" | "xs";
}

export default function Button({
  variant = "default",
  loading = false,
  icon,
  fullWidth = false,
  size,
  children,
  disabled,
  className = "",
  ...rest
}: ButtonProps & { className?: string }) {
  const btnRef = useRef<HTMLButtonElement>(null);

  const classes = [
    "btn",
    `btn--${variant}`,
    size === "sm" ? "btn--sm" : size === "xs" ? "btn--xs" : "btn--lg",
    fullWidth ? "btn--full" : "",
    loading ? "btn--loading" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <button
      ref={btnRef}
      className={classes}
      disabled={disabled || loading}
      {...rest}
    >
      {loading ? (
        <span className="spinner--sm" style={{ display: "flex" }}>
          <span className="spinner__ring" />
        </span>
      ) : (
        <>
          {icon && <span className="btn__icon">{icon}</span>}
          {children}
        </>
      )}
      {variant === "default" && !disabled && !loading && (
        <span className="btn__shine" />
      )}
    </button>
  );
}
