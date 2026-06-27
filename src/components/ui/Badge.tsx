import { type CSSProperties, type ReactNode } from "react";

type BadgeVariant = "default" | "success" | "danger" | "warning" | "info";

interface BadgeProps {
  children: ReactNode;
  variant?: BadgeVariant;
  dot?: boolean;
  compact?: boolean;
  style?: CSSProperties;
  id?: string;
}

const dotClassMap: Record<BadgeVariant, string> = {
  default: "badge__dot--default",
  success: "badge__dot--success",
  danger: "badge__dot--danger",
  warning: "badge__dot--warning",
  info: "badge__dot--info",
};

export default function Badge({
  children,
  variant = "default",
  dot = false,
  style,
  id,
}: BadgeProps) {
  const classes = ["badge", `badge--${variant}`].join(" ");

  return (
    <span id={id} className={classes} style={style}>
      {dot && <span className={`badge__dot ${dotClassMap[variant]}`} />}
      {children}
    </span>
  );
}
