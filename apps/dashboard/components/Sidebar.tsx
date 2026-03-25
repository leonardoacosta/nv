"use client";

import { useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  LayoutDashboard,
  Sun,
  CheckSquare,
  FolderOpen,
  Zap,
  Plug,
  BarChart3,
  Brain,
  Settings,
  Monitor,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";
import NovaMark from "@/components/NovaMark";
import UsageSparkline from "@/components/UsageSparkline";

interface NavItem {
  to: string;
  label: string;
  icon: React.ElementType;
}

const NAV_ITEMS: NavItem[] = [
  { to: "/", label: "Dashboard", icon: LayoutDashboard },
  { to: "/briefing", label: "Briefing", icon: Sun },
  { to: "/obligations", label: "Obligations", icon: CheckSquare },
  { to: "/projects", label: "Projects", icon: FolderOpen },
  { to: "/nexus", label: "Nexus", icon: Zap },
  { to: "/integrations", label: "Integrations", icon: Plug },
  { to: "/usage", label: "Usage", icon: BarChart3 },
  { to: "/memory", label: "Memory", icon: Brain },
  { to: "/session", label: "CC Session", icon: Monitor },
  { to: "/settings", label: "Settings", icon: Settings },
];

export default function Sidebar() {
  const [collapsed, setCollapsed] = useState(false);
  const pathname = usePathname();

  return (
    <aside
      className={[
        "relative flex flex-col bg-cosmic-dark border-r border-cosmic-border",
        "transition-all duration-200 ease-in-out shrink-0",
        collapsed ? "w-16" : "w-56",
      ].join(" ")}
    >
      {/* Nova mark / logo */}
      <div className="flex flex-col border-b border-cosmic-border overflow-hidden">
        <div className="flex items-center gap-3 px-4 py-5">
          <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-cosmic-purple/20 border border-cosmic-purple/30 shrink-0">
            <NovaMark size={20} />
          </div>
          {!collapsed && (
            <span className="text-cosmic-bright font-semibold text-base tracking-tight truncate">
              Nova
            </span>
          )}
        </div>
        {!collapsed && (
          <div className="px-4 pb-3">
            <UsageSparkline />
          </div>
        )}
      </div>

      {/* Navigation */}
      <nav className="flex-1 px-2 py-4 space-y-0.5 overflow-y-auto overflow-x-hidden">
        {NAV_ITEMS.map(({ to, label, icon: Icon }) => {
          const isActive = to === "/" ? pathname === "/" : pathname.startsWith(to);
          return (
            <Link
              key={to}
              href={to}
              className={[
                "flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium",
                "transition-colors duration-150 group",
                isActive
                  ? "bg-cosmic-purple/20 text-cosmic-bright"
                  : "text-cosmic-muted hover:text-cosmic-text hover:bg-cosmic-surface",
              ].join(" ")}
            >
              <Icon
                size={18}
                className={[
                  "shrink-0 transition-colors",
                  isActive
                    ? "text-cosmic-purple"
                    : "text-cosmic-muted group-hover:text-cosmic-text",
                ].join(" ")}
              />
              {!collapsed && (
                <span className="truncate">{label}</span>
              )}
            </Link>
          );
        })}
      </nav>

      {/* Collapse toggle */}
      <button
        type="button"
        onClick={() => setCollapsed((c) => !c)}
        className={[
          "absolute -right-3 top-1/2 -translate-y-1/2 z-10",
          "flex items-center justify-center w-6 h-6 rounded-full",
          "bg-cosmic-surface border border-cosmic-border",
          "text-cosmic-muted hover:text-cosmic-text hover:border-cosmic-purple",
          "transition-colors duration-150 shadow-cosmic-sm",
        ].join(" ")}
        aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
      >
        {collapsed ? <ChevronRight size={12} /> : <ChevronLeft size={12} />}
      </button>
    </aside>
  );
}
