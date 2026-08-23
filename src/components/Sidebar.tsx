import { useNavigate } from "react-router-dom";
import { MessageIcon, GearIcon, LockIcon, HomeIcon, GroupsIcon } from "../components/ui/Icons";

type View = import("../context/AppContext").ViewName;

interface SidebarProps {
  currentView: View;
  onNavigate: (view: View) => void;
}

const items: { id: View; label: string; icon: React.ReactNode }[] = [
  { id: "hub", label: "Chats", icon: <MessageIcon size={18} /> },
  { id: "groups", label: "Groups", icon: <GroupsIcon size={18} /> },
  { id: "settings", label: "Settings", icon: <GearIcon size={18} /> },
];

export default function Sidebar({ currentView, onNavigate }: SidebarProps) {
  const navigate = useNavigate();

  const handleLockVault = async () => {
    try {
      await invoke("lock_vault");
      navigate("/vault");
    } catch (error) {
      console.error("Failed to lock vault:", error);
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
        {items.map((item) => (
          <button
            key={item.id}
            className={`app-sidebar__item ${currentView === item.id ? "app-sidebar__item--active" : ""}`}
            onClick={() => onNavigate(item.id)}
          >
            {item.icon}
            {item.label}
          </button>
        ))}
      </nav>
      <div className="app-sidebar__bottom">
        <button
          className="app-sidebar__item app-sidebar__item--lock"
          onClick={handleLockVault}
        >
          <LockIcon size={18} />
          Lock Vault
        </button>
        <button className="app-sidebar__item" onClick={() => onNavigate("setup")}>
          <HomeIcon size={18} />
          About
        </button>
      </div>
    </aside>
  );
}
