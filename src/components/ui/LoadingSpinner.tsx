interface LoadingSpinnerProps {
  label?: string;
  size?: "sm" | "md" | "lg";
  overlay?: boolean;
}

export default function LoadingSpinner({
  label,
  size = "md",
  overlay = false,
}: LoadingSpinnerProps) {
  const spinner = (
    <div className={`spinner spinner--${size}`}>
      <div className="spinner__ring" />
      {label && <span className="spinner__label">{label}</span>}
    </div>
  );

  if (overlay) {
    return (
      <div className="spinner-overlay">
        {spinner}
      </div>
    );
  }

  return spinner;
}
