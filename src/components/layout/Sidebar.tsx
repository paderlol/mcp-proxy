import { NavLink } from "react-router-dom";
import {
  LayoutDashboard,
  Server,
  KeyRound,
  FileCode,
  Settings,
} from "lucide-react";

const navItems = [
  { to: "/", icon: LayoutDashboard, label: "Dashboard" },
  { to: "/servers", icon: Server, label: "Servers" },
  { to: "/secrets", icon: KeyRound, label: "Secrets" },
  { to: "/config", icon: FileCode, label: "Config" },
  { to: "/settings", icon: Settings, label: "Settings" },
];

export function Sidebar() {
  return (
    <aside className="w-56 h-full bg-bg-base flex flex-col border-r border-border-default/30">
      {/* Logo */}
      <div className="px-5 py-5 flex items-center gap-2.5">
        <div className="w-8 h-8 rounded-full bg-brand flex items-center justify-center">
          <KeyRound size={16} className="text-bg-base" />
        </div>
        <span className="text-base font-bold text-text-primary">
          MCP Proxy
        </span>
      </div>

      {/* Navigation */}
      <nav className="flex-1 px-3 py-2 flex flex-col gap-0.5">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === "/"}
            className={({ isActive }) =>
              `flex items-center gap-3 px-3 py-2.5 rounded-md text-sm transition-colors ${
                isActive
                  ? "text-text-primary font-bold bg-bg-elevated"
                  : "text-text-secondary font-normal hover:text-text-primary hover:bg-bg-elevated/50"
              }`
            }
          >
            <item.icon size={18} />
            {item.label}
          </NavLink>
        ))}
      </nav>

      {/* Footer */}
      <div className="px-5 py-4 border-t border-border-default/30">
        <p className="text-[10px] text-text-secondary/60">v0.1.0</p>
      </div>
    </aside>
  );
}
