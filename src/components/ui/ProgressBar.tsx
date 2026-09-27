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
      {/*
        `role="progressbar"` plus the value attributes, so a file transfer's
        progress is actually announced. The bar was previously a bare styled
        `div` — completely invisible to a screen reader, which matters here
        because a transfer can run for minutes and the user otherwise has no
        way to know it is progressing (or has stalled).
      */}
      <div
        className={classes}
        role="progressbar"
        aria-valuenow={Math.round(percent)}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={label || "Progress"}
        aria-busy={percent < 100}
      >
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
