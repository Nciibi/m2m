import { type IconProps } from "./types";

export function OfflineDot({ size = 8, color = "var(--color-text-muted)" }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 8 8">
      <circle cx="4" cy="4" r="3.5" fill="none" stroke={color} strokeWidth="1" />
    </svg>
  );
}
