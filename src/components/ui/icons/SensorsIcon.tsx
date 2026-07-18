import { type IconProps } from "./types";

export function SensorsIcon({ size = 24, color = "currentColor", className }: IconProps) {
  return (
    <svg className={className} width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="2" fill={color} stroke="none" />
      <path d="M8.5 8.5a5 5 0 0 0 0 7" />
      <path d="M15.5 15.5a5 5 0 0 0 0-7" />
      <path d="M6 6a9 9 0 0 0 0 12" />
      <path d="M18 18a9 9 0 0 0 0-12" />
    </svg>
  );
}
