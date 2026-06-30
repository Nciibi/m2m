import { type CSSProperties } from "react";

interface ProgressBarProps {
  value: number; // 0-100
  max?: number;
  variant?: "default" | "success" | "danger" | "warning";
  size?: "default" | "small";
  showLabel?: boolean;
  label?: string;
  className?: string;
  style?: CSSProperties;
}

export default function ProgressBar({
  value,
  max = 100,
  variant = "default",
  size = "default",
  showLabel = false,
  label,
  className = "",
  style,
}: ProgressBarProps) {
  const percent = Math.min(100, Math.max(0, (value / max) * 100));

  const classes = [
    "progress-bar",
    size === "small" ? "progress-bar--small" : "",
    variant !== "default" ? `progress-bar--${variant}` : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className="progress-container" style={style}>
      <div className={classes}>
        <div className="progress-bar__fill" style={{ width: `${percent}%` }} />
      </div>
      {showLabel && (
        <div className="progress-info">
          {label && <span className="progress-info__label">{label}</span>}
          <span className="progress-info__value">{Math.round(percent)}%</span>
        </div>
      )}
    </div>
  );
}
