import { type IconProps } from "./types";

export function OnlineDot({ size = 8, color = "var(--color-success)" }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 8 8">
      <circle cx="4" cy="4" r="4" fill={color} />
    </svg>
  );
}
