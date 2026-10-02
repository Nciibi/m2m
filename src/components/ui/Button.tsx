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
  // Defaults to "button", not the HTML implicit "submit".
  //
  // `<button>` inside a `<form>` submits that form unless `type` says
  // otherwise, so every Button was a latent submit trigger. Nothing here is
  // currently inside a `<form>`, which is exactly why it had never fired — the
  // bug surfaces the day someone wraps a modal body in a `<form>`, which is a
  // normal thing to do. `FamilyTab`'s save handler is even typed
  // `React.MouseEvent | React.FormEvent` in anticipation.
  //
  // A caller can still pass `type="submit"` explicitly; the default only applies
  // when they do not.
  type = "button",
  ...rest
}: ButtonProps & { className?: string }) {
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
      type={type}
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
