import { type IconProps } from "./types";

export function CheckDoubleIcon({ size = 24, color = "currentColor", className }: IconProps) {
  return (
    <svg className={className} width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="2 12 7 17 13 7" />
      <polyline points="9 12 14 17 22 7" />
    </svg>
  );
}
