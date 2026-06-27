import { type CSSProperties, type ReactNode } from "react";

interface CardProps {
  children: ReactNode;
  /** Card header with icon + title */
  header?: { icon: string; title: string; iconVariant?: "accent" | "success" | "warning" | "danger" };
  /** Description text below header */
  description?: string;
  /** If true, card acts as a clickable button */
  clickable?: boolean;
  /** Click handler (only used when clickable) */
  onClick?: () => void;
  /** Optional style overrides */
  style?: CSSProperties;
  /** Optional class name */
  className?: string;
  /** Card ID */
  id?: string;
}

const iconBgMap: Record<string, CSSProperties> = {
  accent: { background: "var(--color-accent-glow)", border: "1px solid rgba(129,140,248,0.15)" },
  success: { background: "var(--color-success-glow)", border: "1px solid rgba(52,211,153,0.15)" },
  warning: { background: "var(--color-warning-glow)", border: "1px solid rgba(251,191,36,0.15)" },
  danger: { background: "var(--color-danger-glow)", border: "1px solid rgba(248,113,113,0.15)" },
};

/**
 * Glass card container used throughout M2M for groupings of controls.
 */
export default function Card({
  children,
  header,
  description,
  clickable = false,
  onClick,
  style,
  className,
  id,
}: CardProps) {
  const cardStyle: CSSProperties = {
    background: "var(--color-bg-card)",
    border: "1px solid rgba(255,255,255,0.04)",
    padding: "var(--space-2xl)",
    borderRadius: "var(--radius-xl)",
    display: "flex",
    flexDirection: "column",
    gap: "var(--space-md)",
    transition: "var(--transition-base)",
    position: "relative",
    overflow: "hidden",
    boxShadow: "var(--shadow-card)",
    cursor: clickable ? "pointer" : undefined,
    ...(clickable && {
      ":hover": {
        borderColor: "var(--color-border-strong)",
        background: "rgba(35,36,50,0.7)",
        transform: "translateY(-4px)",
        boxShadow: "var(--shadow-lg)",
      },
    }),
    ...style,
  };

  const content = (
    <div
      id={id}
      className={`card ${className || ""}`}
      style={cardStyle}
      onClick={clickable ? onClick : undefined}
      role={clickable ? "button" : undefined}
      tabIndex={clickable ? 0 : undefined}
      onKeyDown={clickable ? (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onClick?.(); } } : undefined}
    >
      {/* Top highlight line */}
      <div
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          right: 0,
          height: 1,
          background: "linear-gradient(90deg, transparent, rgba(255,255,255,0.1), transparent)",
          pointerEvents: "none",
        }}
      />

      {header && (
        <div style={{ display: "flex", alignItems: "center", gap: "var(--space-sm)" }}>
          <div
            style={{
              width: 36,
              height: 36,
              borderRadius: 10,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontSize: "1rem",
              flexShrink: 0,
              ...iconBgMap[header.iconVariant || "accent"],
            }}
          >
            {header.icon}
          </div>
          <h3 style={{ margin: 0, fontSize: "var(--text-lg)", fontWeight: 600 }}>
            {header.title}
          </h3>
        </div>
      )}

      {description && (
        <p style={{ margin: 0, fontSize: "var(--text-base)", color: "var(--color-text-secondary)", lineHeight: 1.5 }}>
          {description}
        </p>
      )}

      {children}
    </div>
  );

  // Hover effect via inline style on mouse events
  if (clickable) {
    return (
      <div
        onMouseEnter={(e) => {
          const t = e.currentTarget.querySelector(".card") as HTMLElement;
          if (t) {
            t.style.borderColor = "var(--color-border-strong)";
            t.style.background = "rgba(35,36,50,0.7)";
            t.style.transform = "translateY(-4px)";
            t.style.boxShadow = "var(--shadow-lg)";
          }
        }}
        onMouseLeave={(e) => {
          const t = e.currentTarget.querySelector(".card") as HTMLElement;
          if (t) {
            t.style.borderColor = "rgba(255,255,255,0.04)";
            t.style.background = "var(--color-bg-card)";
            t.style.transform = "";
            t.style.boxShadow = "var(--shadow-md)";
          }
        }}
        style={{ all: "initial" }}
      >
        {content}
      </div>
    );
  }

  return content;
}
