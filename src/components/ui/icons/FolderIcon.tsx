import { type IconProps } from "./types";

export function FolderIcon({ size = 24, color = "currentColor", className, open = false }: IconProps & { open?: boolean }) {
  return (
    <svg className={className} width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      {open ? (
        <>
          <path d="M4 20h16a2 2 0 0 0 2-2v-6a2 2 0 0 0-2-2h-9.17a2 2 0 0 1-1.66-.89l-.83-1.22H4a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2z" />
          <path d="M2 10l2.4-5.2A1.5 1.5 0 0 1 5.76 4h4.06a1.5 1.5 0 0 1 1.36.86L12 6" />
        </>
      ) : (
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
      )}
    </svg>
  );
}
