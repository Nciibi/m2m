import { invoke } from "@tauri-apps/api/core";
import { MessageIcon, GearIcon, LockIcon, GroupsIcon } from "./ui/Icons";
import { errorMessage } from "../utils";

type View = import("../context/AppContext").ViewName;

interface SidebarProps {
  currentView: View;
  onNavigate: (view: View) => void;
  /**
   * Report a failed action to the user.
   *
   * The lock handler used to write its failure to `console.error` and show
   * nothing. That is verbatim the bug CLAUDE.md records as fixed elsewhere
   * ("a silent catch here meant that if `lock_vault` failed, the user believed
   * their vault had auto-locked and it had not") — the fix landed in `App.tsx`
   * and in the Settings handler, and missed the one on every screen.
   */
  onError?: (message: string) => void;
}

const items: { id: View; label: string; icon: React.ReactNode }[] = [
  { id: "hub", label: "Chats", icon: <MessageIcon size={18} /> },
  { id: "groups", label: "Groups", icon: <GroupsIcon size={18} /> },
  { id: "settings", label: "Settings", icon: <GearIcon size={18} /> },
];

/** Views that should highlight the Chats entry. */
const HUB_VIEWS: View[] = ["hub", "chat"];

export default function Sidebar({ currentView, onNavigate, onError }: SidebarProps) {
  const handleLockVault = async () => {
    try {
      await invoke("lock_vault");
      // Routing back to the unlock screen; state cleanup happens in lock_vault.
      onNavigate("vault");
    } catch (error) {
      // Never swallow this: the user must not walk away believing the vault is
      // locked when the command rejected and the keys are still resident.
      const message = "Vault lock FAILED: " + errorMessage(error, "unknown error");
      if (onError) onError(message);
      else console.error(message);
    }
  };

  return (
    <aside className="app-sidebar">
      <div className="app-sidebar__brand">
        <div className="app-sidebar__logo">
          <LockIcon size={16} color="white" />
        </div>
        <div>
          <div className="app-sidebar__title">M2M</div>
          <div className="app-sidebar__subtitle">Secure</div>
        </div>
      </div>
      <nav className="app-sidebar__nav" aria-label="Main navigation">
        {items.map((item) => {
          const active =
            item.id === "hub"
              ? HUB_VIEWS.includes(currentView)
              : currentView === item.id;
          return (
            <button
              key={item.id}
              className={`app-sidebar__item ${active ? "app-sidebar__item--active" : ""}`}
              onClick={() => onNavigate(item.id)}
              aria-current={active ? "page" : undefined}
            >
              {item.icon}
              {item.label}
            </button>
          );
        })}
      </nav>
      <div className="app-sidebar__bottom">
        <button className="app-sidebar__item" onClick={handleLockVault}>
          <LockIcon size={18} />
          Lock Vault
        </button>
      </div>
    </aside>
  );
}
