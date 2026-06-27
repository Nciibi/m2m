import { type CSSProperties, type ReactNode } from "react";

type BadgeVariant = "default" | "success" | "danger" | "warning" | "info";

interface BadgeProps {
  children: string;
  variant?: BadgeVariant;
  /** If true, shows a pulsing dot before the text */
  dot?: boolean;
  /** Small size */
  compact?: boolean;
  style?: CSSProperties;
  id?: string;
}

const variantStyles: Record<BadgeVariant, CSSProperties> = {
  default: {
    background: "var(--color-bg-input)",
    color: "var(--color-text-secondary)",
    borderColor: "var(--color-border-default)",
  },
  success: {
    background: "var(--color-success-glow)",
    color: "var(--color-success)",
    borderColor: "rgba(16,185,129,0.25)",
  },
  danger: {
    background: "var(--color-danger-glow)",
    color: "var(--color-danger)",
    borderColor: "rgba(239,68,68,0.2)",
  },
  warning: {
    background: "var(--color-warning-glow)",
    color: "var(--color-warning)",
    borderColor: "rgba(245,158,11,0.2)",
  },
  info: {
    background: "var(--color-accent-glow-subtle)",
    color: "var(--color-accent-bright)",
    borderColor: "rgba(129,140,248,0.2)",
  },
};

const dotColorMap: Record<BadgeVariant, string> = {
  default: "var(--color-text-muted)",
  success: "var(--color-success)",
  danger: "var(--color-danger)",
  warning: "var(--color-warning)",
  info: "var(--color-accent)",
};

/**
 * Inline status badge with optional pulsing dot indicator.
 * Used for connection state, NAT type, encryption status, etc.
 */
export default function Badge({
  children,
  variant = "default",
  dot = false,
  compact = false,
  style,
  id,
}: BadgeProps) {
  const s = variantStyles[variant];

  return (
    <span
      id={id}
      className={`badge badge-${variant}`}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        fontSize: compact ? "var(--text-xs)" : "var(--text-sm)",
        padding: compact ? "3px 10px" : "5px 12px",
        borderRadius: "var(--radius-full)",
        fontWeight: 500,
        textTransform: "uppercase",
        letterSpacing: "var(--letter-spacing-uppercase)",
        border: "1px solid",
        lineHeight: 1.3,
        whiteSpace: "nowrap",
        ...s,
        ...style,
      }}
    >
      {dot && (
        <span
          style={{
            width: 6,
            height: 6,
            borderRadius: "50%",
            background: dotColorMap[variant],
            boxShadow: variant === "success"
              ? "0 0 8px var(--color-success-glow)"
              : undefined,
            animation: variant === "success"
              ? "pulse 2s ease-in-out infinite"
              : undefined,
            flexShrink: 0,
          }}
        />
      )}
      {children}
    </span>
  );
}
