import { type CSSProperties, type ReactNode, useRef } from "react";

interface CardProps {
  children: ReactNode;
  header?: { icon: ReactNode; title: string; iconVariant?: "accent" | "success" | "warning" | "danger" };
  description?: string;
  clickable?: boolean;
  onClick?: () => void;
  style?: CSSProperties;
  className?: string;
  id?: string;
}

const iconClasses: Record<string, string> = {
  accent: "card__icon--accent",
  success: "card__icon--success",
  warning: "card__icon--warning",
  danger: "card__icon--danger",
};

export default function Card({
  children,
  header,
  description,
  clickable = false,
  onClick,
  style,
  className = "",
  id,
}: CardProps) {
  const cardRef = useRef<HTMLDivElement>(null);
  const classes = [
    "card",
    clickable ? "card--clickable" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      id={id}
      ref={cardRef}
      className={classes}
      style={style}
      onClick={clickable ? onClick : undefined}
      role={clickable ? "button" : undefined}
      tabIndex={clickable ? 0 : undefined}
      onKeyDown={
        clickable
          ? (e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onClick?.();
              }
            }
          : undefined
      }
    >
      {header && (
        <div className="card__header">
          <div className={`card__icon ${iconClasses[header.iconVariant || "accent"] || iconClasses.accent}`}>
            {header.icon}
          </div>
          <h3 className="card__title">{header.title}</h3>
        </div>
      )}
      {description && <p className="card__desc">{description}</p>}
      {children}
    </div>
  );
}
