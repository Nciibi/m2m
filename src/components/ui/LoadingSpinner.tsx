import { type CSSProperties } from "react";

interface LoadingSpinnerProps {
  label?: string;
  size?: "sm" | "md" | "lg";
  overlay?: boolean;
  style?: CSSProperties;
}

export default function LoadingSpinner({
  label,
  size = "md",
  overlay = false,
  style,
}: LoadingSpinnerProps) {
  const spinner = (
    <div className={`spinner spinner--${size}`} style={style}>
      <div className="spinner__ring" />
      {label && <span className="spinner__label">{label}</span>}
    </div>
  );

  if (overlay) {
    return (
      <div
        style={{
          position: "fixed",
          inset: 0,
          zIndex: "var(--z-modal)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          background: "var(--color-bg-overlay)",
          backdropFilter: "blur(4px)",
        }}
      >
        {spinner}
      </div>
    );
  }

  return spinner;
}
