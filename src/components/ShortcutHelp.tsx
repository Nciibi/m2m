import { Modal } from "./ui";

interface Shortcut {
  key: string;
  desc: string;
}

const SHORTCUTS: Shortcut[] = [
  { key: "Esc", desc: "Go back to hub (from chat)" },
  { key: "Ctrl+,", desc: "Open settings" },
  { key: "Ctrl+Enter", desc: "Send message" },
  { key: "?", desc: "Toggle this help modal" },
];

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function ShortcutHelp({ open, onClose }: Props) {
  return (
    <Modal open={open} onClose={onClose} title="Keyboard Shortcuts">
      <div className="shortcut-list">
        {SHORTCUTS.map((s) => (
          <div key={s.key} className="shortcut-row">
            <kbd className="shortcut-key">{s.key}</kbd>
            <span className="shortcut-desc">{s.desc}</span>
          </div>
        ))}
      </div>
    </Modal>
  );
}
