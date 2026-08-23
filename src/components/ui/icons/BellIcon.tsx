import { type IconProps } from "./types";

export function BellIcon({ size = 24, color = "currentColor", className, off = false }: IconProps & { off?: boolean }) {
  return (
    <svg className={className} width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9" />
      <path d="M13.73 21a2 2 0 0 1-3.46 0" />
      {off && <line x1="3" y1="3" x2="21" y2="21" />}
    </svg>
  );
}
