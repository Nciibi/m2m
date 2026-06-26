import { type CSSProperties } from "react";

interface LoadingSpinnerProps {
  /** Small label shown below the spinner */
  label?: string;
  /** Size variant */
  size?: "sm" | "md" | "lg";
  /** If true, renders as a fullscreen overlay */
  overlay?: boolean;
  /** Optional inline style overrides */
  style?: CSSProperties;
}

const sizeMap = {
  sm: 16,
  md: 24,
  lg: 40,
};

const borderMap = {
  sm: 2,
  md: 3,
  lg: 4,
};

/**
 * A spinning indicator used throughout M2M for async operations.
 * Respects prefers-reduced-motion.
 */
export default function LoadingSpinner({
  label,
  size = "md",
  overlay = false,
  style,
}: LoadingSpinnerProps) {
  const px = sizeMap[size];
  const border = borderMap[size];

  const spinner = (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 12,
        ...style,
      }}
    >
      <div
        className="loading-spinner-ring"
        style={{
          width: px,
          height: px,
          border: `${border}px solid var(--color-border-default)`,
          borderTopColor: "var(--color-accent)",
          borderRadius: "50%",
          animation: "spin 0.8s linear infinite",
        }}
      />
      {label && (
        <span
          style={{
            fontSize: "var(--text-base)",
            color: "var(--color-text-muted)",
            fontWeight: 500,
          }}
        >
          {label}
        </span>
      )}
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
