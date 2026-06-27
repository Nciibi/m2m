import { type ButtonHTMLAttributes, type ReactNode } from "react";
import LoadingSpinner from "./LoadingSpinner";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** Visual variant */
  variant?: "default" | "secondary" | "danger" | "ghost" | "icon";
  /** Show a loading spinner instead of children */
  loading?: boolean;
  /** Icon displayed before text (not for icon variant) */
  icon?: ReactNode;
  /** Full-width button */
  fullWidth?: boolean;
  /** Small size (fits in tight spaces) */
  compact?: boolean;
}

const variantStyles: Record<string, React.CSSProperties> = {
  default: {
    background: "linear-gradient(135deg, var(--color-accent), var(--color-accent-dim))",
    color: "white",
    border: "1px solid rgba(255,255,255,0.1)",
    boxShadow: "var(--shadow-accent)",
  },
  secondary: {
    background: "var(--color-bg-input)",
    color: "var(--color-text-secondary)",
    border: "1px solid var(--color-border-default)",
    boxShadow: "none",
  },
  danger: {
    background: "transparent",
    color: "var(--color-danger)",
    border: "1px solid rgba(239,68,68,0.3)",
    boxShadow: "none",
  },
  ghost: {
    background: "transparent",
    color: "var(--color-text-secondary)",
    border: "1px solid transparent",
    boxShadow: "none",
  },
  icon: {
    background: "transparent",
    color: "var(--color-text-secondary)",
    border: "1px solid var(--color-border-default)",
    boxShadow: "none",
    padding: "10px",
    minWidth: "42px",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  },
};

/**
 * Unified button component with variants, loading state, and decorative shine effect.
 */
export default function Button({
  variant = "default",
  loading = false,
  icon,
  fullWidth = false,
  compact = false,
  children,
  disabled,
  style,
  ...rest
}: ButtonProps) {
  const baseStyle: React.CSSProperties = {
    fontFamily: "inherit",
    fontSize: compact ? "var(--text-base)" : "var(--text-md)",
    fontWeight: 600,
    cursor: disabled || loading ? "not-allowed" : "pointer",
    padding: compact ? "7px 14px" : variant === "icon" ? "10px" : "12px 24px",
    borderRadius: "var(--radius-lg)",
    transition: "var(--transition-base)",
    position: "relative",
    overflow: "hidden",
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    gap: 8,
    opacity: disabled ? 0.5 : 1,
    width: fullWidth ? "100%" : undefined,
    ...variantStyles[variant],
    ...style,
  };

  return (
    <button
      style={baseStyle}
      disabled={disabled || loading}
      onMouseEnter={(e) => {
        if (!disabled && !loading) {
          const target = e.currentTarget;
          if (variant === "default") {
            target.style.transform = "translateY(-2px)";
            target.style.boxShadow = "var(--shadow-accent-strong)";
          } else if (variant === "secondary" || variant === "ghost") {
            target.style.background = "var(--color-bg-hover)";
            target.style.color = "var(--color-text-primary)";
          } else if (variant === "danger") {
            target.style.background = "var(--color-danger-bg)";
          } else if (variant === "icon") {
            target.style.background = "var(--color-bg-hover)";
            target.style.borderColor = "var(--color-border-active)";
            target.style.color = "var(--color-text-primary)";
          }
        }
      }}
      onMouseLeave={(e) => {
        const target = e.currentTarget;
        target.style.transform = "";
        const base = variantStyles[variant];
        target.style.boxShadow = base.boxShadow as string || "";
        target.style.background = base.background as string || "";
        target.style.color = base.color as string || "";
        target.style.borderColor = base.border as string || "";
      }}
      onMouseDown={(e) => {
        if (!disabled && !loading) {
          e.currentTarget.style.transform = "translateY(0)";
        }
      }}
      {...rest}
    >
      {loading ? (
        <LoadingSpinner size="sm" />
      ) : (
        <>
          {icon && <span style={{ fontSize: "1.1rem", lineHeight: 1 }}>{icon}</span>}
          {children}
        </>
      )}
      {/* Shine sweep effect (only on default variant) */}
      {variant === "default" && !disabled && !loading && (
        <span
          className="btn-shine"
          style={{
            position: "absolute",
            top: 0,
            left: "-100%",
            right: 0,
            bottom: 0,
            background: "linear-gradient(90deg, transparent, rgba(255,255,255,0.2), transparent)",
            transition: "left 0.5s",
            pointerEvents: "none",
          }}
          onMouseEnter={(e) => { e.currentTarget.style.left = "100%"; }}
          onMouseLeave={(e) => { e.currentTarget.style.left = "-100%"; }}
        />
      )}
    </button>
  );
}
