import React, { ReactNode, forwardRef } from "react";
import "./FocusRing.css";

export interface FocusRingProps {
  children: ReactNode;
  className?: string;
}

const FocusRing = forwardRef<HTMLDivElement, FocusRingProps>(function FocusRing(
  { children, className = "" },
  ref,
) {
  return (
    <div
      ref={ref}
      className={`focus-ring-wrapper ${className}`}
      tabIndex={-1}
    >
      {children}
    </div>
  );
});

export default FocusRing;
