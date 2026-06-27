import { type ButtonHTMLAttributes, type ReactNode, useState } from "react";
import LoadingSpinner from "./LoadingSpinner";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "default" | "secondary" | "danger" | "ghost" | "icon";
  loading?: boolean;
  icon?: ReactNode;
  fullWidth?: boolean;
  compact?: boolean;
}

const variantStyles: Record<string, React.CSSProperties> = {
  default: {
    background: "var(--color-accent-gradient)",
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
    border: "1px solid rgba(239,68,68,0.25)",
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
    minHeight: "42px",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    borderRadius: "var(--radius-lg)",
    fontSize: "var(--text-xl)",
  },
};

/**
 * Unified button component with variants, loading state, and shine sweep effect.
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
  const [isHovered, setIsHovered] = useState(false);

  const baseStyle: React.CSSProperties = {
    fontFamily: "inherit",
    fontSize: compact ? "var(--text-base)" : "var(--text-md)",
    fontWeight: 600,
    cursor: disabled || loading ? "not-allowed" : "pointer",
    padding: compact
      ? "7px 14px"
      : variant === "icon"
        ? "10px"
        : "12px 24px",
    borderRadius: variant === "icon" ? "var(--radius-lg)" : "var(--radius-lg)",
    transition: "var(--transition-base)",
    position: "relative",
    overflow: "hidden",
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    gap: 8,
    opacity: disabled ? 0.45 : 1,
    width: fullWidth ? "100%" : undefined,
    ...variantStyles[variant],
    ...style,
  };

  return (
    <button
      style={baseStyle}
      disabled={disabled || loading}
      onMouseEnter={(e) => {
        setIsHovered(true);
        if (!disabled && !loading) {
          const t = e.currentTarget;
          if (variant === "default") {
            t.style.transform = "translateY(-2px)";
            t.style.boxShadow = "var(--shadow-accent-strong)";
          } else if (variant === "secondary" || variant === "ghost") {
            t.style.background = "var(--color-bg-hover)";
            t.style.color = "var(--color-text-primary)";
          } else if (variant === "danger") {
            t.style.background = "var(--color-danger-bg)";
          } else if (variant === "icon") {
            t.style.background = "var(--color-bg-hover)";
            t.style.borderColor = "var(--color-border-active)";
            t.style.color = "var(--color-text-primary)";
          }
        }
      }}
      onMouseLeave={(e) => {
        setIsHovered(false);
        const t = e.currentTarget;
        t.style.transform = "";
        const base = variantStyles[variant];
        t.style.boxShadow = (base.boxShadow as string) || "";
        t.style.background = (base.background as string) || "";
        t.style.color = (base.color as string) || "";
        t.style.borderColor = (base.border as string) || "";
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
          {icon && (
            <span style={{ fontSize: "1.1rem", lineHeight: 1 }}>
              {icon}
            </span>
          )}
          {children}
        </>
      )}
      {/* Shine sweep on default variant */}
      {variant === "default" && !disabled && !loading && (
        <span
          style={{
            position: "absolute",
            top: 0,
            left: isHovered ? "100%" : "-100%",
            right: 0,
            bottom: 0,
            background:
              "linear-gradient(90deg, transparent, rgba(255,255,255,0.2), transparent)",
            transition: "left 0.5s",
            pointerEvents: "none",
          }}
        />
      )}
    </button>
  );
}
