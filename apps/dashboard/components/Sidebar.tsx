"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  LayoutDashboard,
  Sun,
  CheckSquare,
  BookOpen,
  FolderOpen,
  Plug,
  BarChart3,
  Brain,
  Settings,
  Layers,
  MessageSquare,
  Users,
  ChevronLeft,
  ChevronRight,
  ShieldAlert,
  Menu,
  X,
} from "lucide-react";
import NovaMark from "@/components/NovaMark";
import UsageSparkline from "@/components/UsageSparkline";
import {
  useDaemonEvents,
  useDaemonStatus,
  type WsStatus,
} from "@/components/providers/DaemonEventContext";

// ---------------------------------------------------------------------------
// WebSocket status footer
// ---------------------------------------------------------------------------

const WS_STATUS_CONFIG: Record<
  WsStatus,
  { dot: string; label: string; text: string }
> = {
  connected: {
    dot: "bg-green-700",
    label: "Daemon connected",
    text: "Connected",
  },
  reconnecting: {
    dot: "bg-amber-700 animate-pulse",
    label: "Daemon reconnecting",
    text: "Reconnecting…",
  },
  disconnected: {
    dot: "bg-red-700",
    label: "Daemon disconnected",
    text: "Disconnected",
  },
};

function WsStatusFooter({ collapsed }: { collapsed: boolean }) {
  const status = useDaemonStatus();
  const cfg = WS_STATUS_CONFIG[status];
  return (
    <div
      className="flex items-center gap-2 px-3 py-2.5 min-h-11"
      title={cfg.label}
    >
      <span
        className={`inline-block w-1.5 h-1.5 rounded-full shrink-0 ${cfg.dot}`}
        aria-label={cfg.label}
        role="img"
      />
      {!collapsed && (
        <span className="text-label-12 text-ds-gray-700 truncate">{cfg.text}</span>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Approvals badge
// ---------------------------------------------------------------------------

interface Obligation {
  id: string;
  status: string;
  owner: string;
}

function useApprovalCount(): number {
  const [count, setCount] = useState(0);

  const fetchCount = useCallback(async () => {
    try {
      const res = await fetch("/api/obligations?owner=leo&status=open");
      if (!res.ok) return;
      const data = (await res.json()) as Obligation[];
      setCount(Array.isArray(data) ? data.length : 0);
    } catch {
      // silently swallow fetch errors for badge
    }
  }, []);

  useEffect(() => {
    void fetchCount();
  }, [fetchCount]);

  useDaemonEvents(
    useCallback(
      (_ev) => {
        void fetchCount();
      },
      [fetchCount],
    ),
    "approval",
  );

  return count;
}

// ---------------------------------------------------------------------------
// Nav items with section groups
// ---------------------------------------------------------------------------

interface NavItem {
  to: string;
  label: string;
  icon: React.ElementType;
  badge?: "approvals";
}

interface NavGroup {
  label: string;
  items: NavItem[];
}

const NAV_GROUPS: NavGroup[] = [
  {
    label: "Overview",
    items: [
      { to: "/", label: "Dashboard", icon: LayoutDashboard },
      { to: "/briefing", label: "Briefing", icon: Sun },
    ],
  },
  {
    label: "Activity",
    items: [
      { to: "/obligations", label: "Obligations", icon: CheckSquare },
      { to: "/approvals", label: "Approvals", icon: ShieldAlert, badge: "approvals" },
      { to: "/diary", label: "Diary", icon: BookOpen },
      { to: "/sessions", label: "Sessions", icon: Layers },
      { to: "/messages", label: "Messages", icon: MessageSquare },
    ],
  },
  {
    label: "Data",
    items: [
      { to: "/contacts", label: "Contacts", icon: Users },
      { to: "/projects", label: "Projects", icon: FolderOpen },
      { to: "/memory", label: "Memory", icon: Brain },
      { to: "/integrations", label: "Integrations", icon: Plug },
    ],
  },
  {
    label: "System",
    items: [
      { to: "/usage", label: "Usage", icon: BarChart3 },
      { to: "/settings", label: "Settings", icon: Settings },
    ],
  },
];

// ---------------------------------------------------------------------------
// NavLink
// ---------------------------------------------------------------------------

interface NavLinkProps {
  item: NavItem;
  collapsed: boolean;
  isActive: boolean;
  approvalCount: number;
  onClick?: () => void;
}

function NavLink({ item, collapsed, isActive, approvalCount, onClick }: NavLinkProps) {
  const { to, label, icon: Icon, badge } = item;
  const badgeCount = badge === "approvals" ? approvalCount : 0;

  return (
    <Link
      href={to}
      onClick={onClick}
      className={[
        "flex items-center gap-3 px-3 py-2 min-h-9 rounded-md text-label-14",
        "transition-colors duration-150 group relative",
        isActive
          ? "bg-ds-gray-alpha-200 text-ds-gray-1000 border-l-2 border-ds-gray-1000"
          : "text-ds-gray-900 hover:text-ds-gray-1000 hover:bg-ds-gray-alpha-100",
      ].join(" ")}
      style={isActive ? { paddingLeft: "10px" } : undefined}
    >
      <Icon
        size={18}
        className={[
          "shrink-0 transition-colors",
          isActive
            ? "text-ds-gray-1000"
            : "text-ds-gray-700 group-hover:text-ds-gray-1000",
        ].join(" ")}
      />
      {!collapsed && <span className="truncate flex-1">{label}</span>}

      {/* Approval badge */}
      {badgeCount > 0 && (
        <span
          className={[
            "inline-flex items-center justify-center rounded-full text-xs font-mono font-bold",
            "bg-amber-700/20 text-amber-700 border border-amber-700/30",
            collapsed
              ? "absolute -top-0.5 -right-0.5 w-4 h-4 text-[10px]"
              : "px-1.5 py-0.5 min-w-[1.25rem] shrink-0",
          ].join(" ")}
          aria-label={`${badgeCount} pending approvals`}
        >
          {badgeCount > 99 ? "99+" : badgeCount}
        </span>
      )}
    </Link>
  );
}

// ---------------------------------------------------------------------------
// SidebarContent — shared nav tree used by both desktop and mobile drawer
// ---------------------------------------------------------------------------

interface SidebarContentProps {
  collapsed: boolean;
  pathname: string;
  approvalCount: number;
  onNavClick?: () => void;
}

function SidebarContent({ collapsed, pathname, approvalCount, onNavClick }: SidebarContentProps) {
  return (
    <>
      {/* Logo */}
      <div
        className="flex flex-col overflow-hidden shrink-0"
        style={{ borderBottom: "1px solid var(--ds-gray-alpha-200)" }}
      >
        <div className="flex items-center gap-3 px-4 py-4">
          <div
            className="flex items-center justify-center w-7 h-7 rounded-md shrink-0"
            style={{ background: "var(--ds-gray-alpha-200)" }}
          >
            <NovaMark size={18} />
          </div>
          {!collapsed && (
            <span className="text-label-16 font-semibold text-ds-gray-1000 truncate">
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

      {/* Navigation with section groups */}
      <nav className="flex-1 px-2 py-3 overflow-y-auto overflow-x-hidden">
        {NAV_GROUPS.map((group, groupIndex) => (
          <div key={group.label}>
            {/* Group separator — not before first group */}
            {groupIndex > 0 && (
              <div
                className="my-2 mx-1"
                style={{ borderBottom: "1px solid var(--ds-gray-alpha-200)" }}
              />
            )}

            {/* Group label */}
            {!collapsed && (
              <div className="px-2 pt-2 pb-1">
                <span className="text-label-12 text-ds-gray-700">
                  {group.label}
                </span>
              </div>
            )}

            {/* Group items */}
            <div className="space-y-0.5">
              {group.items.map((item) => {
                const isActive =
                  item.to === "/"
                    ? pathname === "/"
                    : pathname.startsWith(item.to);
                return (
                  <NavLink
                    key={item.to}
                    item={item}
                    collapsed={collapsed}
                    isActive={isActive}
                    approvalCount={approvalCount}
                    onClick={onNavClick}
                  />
                );
              })}
            </div>
          </div>
        ))}
      </nav>

      {/* Footer: WebSocket status */}
      <div
        className="shrink-0"
        style={{ borderTop: "1px solid var(--ds-gray-alpha-200)" }}
      >
        <WsStatusFooter collapsed={collapsed} />
      </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// Sidebar — desktop + mobile
// ---------------------------------------------------------------------------

export default function Sidebar() {
  const [collapsed, setCollapsed] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const pathname = usePathname();
  const approvalCount = useApprovalCount();
  const drawerRef = useRef<HTMLDivElement>(null);

  // Close mobile drawer on route change
  useEffect(() => {
    setMobileOpen(false);
  }, [pathname]);

  // Close on Escape key
  useEffect(() => {
    if (!mobileOpen) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMobileOpen(false);
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [mobileOpen]);

  return (
    <>
      {/* ------------------------------------------------------------------ */}
      {/* Mobile: hamburger button (visible at ≤640px)                        */}
      {/* ------------------------------------------------------------------ */}
      <button
        type="button"
        onClick={() => setMobileOpen(true)}
        aria-label="Open navigation"
        className={[
          "sm:hidden fixed top-3 left-3 z-40",
          "flex items-center justify-center w-11 h-11 rounded-md",
          "bg-ds-bg-200 border border-ds-gray-400 text-ds-gray-900",
          "hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors",
          "shadow-sm",
        ].join(" ")}
      >
        <Menu size={18} />
      </button>

      {/* ------------------------------------------------------------------ */}
      {/* Mobile: overlay drawer (visible at ≤768px when open)                */}
      {/* ------------------------------------------------------------------ */}
      {mobileOpen && (
        <div
          className="md:hidden fixed inset-0 z-50 flex"
          role="dialog"
          aria-modal="true"
          aria-label="Navigation"
        >
          {/* Scrim */}
          <div
            className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            onClick={() => setMobileOpen(false)}
          />

          {/* Drawer */}
          <div
            ref={drawerRef}
            className="relative z-10 flex flex-col w-64 bg-ds-bg-200 border-r border-ds-gray-400"
          >
            {/* Close button */}
            <button
              type="button"
              onClick={() => setMobileOpen(false)}
              aria-label="Close navigation"
              className="absolute top-3 right-3 flex items-center justify-center w-9 h-9 rounded-md text-ds-gray-900 hover:text-ds-gray-1000 hover:bg-ds-gray-alpha-100 transition-colors"
            >
              <X size={16} />
            </button>

            <SidebarContent
              collapsed={false}
              pathname={pathname}
              approvalCount={approvalCount}
              onNavClick={() => setMobileOpen(false)}
            />
          </div>
        </div>
      )}

      {/* ------------------------------------------------------------------ */}
      {/* Tablet (641–768px): icon-only rail                                  */}
      {/* ------------------------------------------------------------------ */}
      <aside
        className="hidden sm:flex md:hidden relative flex-col bg-ds-bg-200 border-r border-ds-gray-400 w-16 shrink-0"
        aria-label="Navigation rail"
      >
        <SidebarContent
          collapsed={true}
          pathname={pathname}
          approvalCount={approvalCount}
        />
      </aside>

      {/* ------------------------------------------------------------------ */}
      {/* Desktop (≥768px): collapsible full sidebar                          */}
      {/* ------------------------------------------------------------------ */}
      <aside
        className={[
          "hidden md:relative md:flex flex-col bg-ds-bg-200 border-r border-ds-gray-400",
          "transition-all duration-200 ease-in-out shrink-0",
          collapsed ? "w-16" : "w-56",
        ].join(" ")}
        aria-label="Main navigation"
      >
        <SidebarContent
          collapsed={collapsed}
          pathname={pathname}
          approvalCount={approvalCount}
        />

        {/* Collapse toggle */}
        <button
          type="button"
          onClick={() => setCollapsed((c) => !c)}
          className={[
            "absolute -right-3 top-1/2 -translate-y-1/2 z-10",
            "flex items-center justify-center w-6 h-6 rounded-full",
            "bg-ds-bg-200 border border-ds-gray-400",
            "text-ds-gray-700 hover:text-ds-gray-1000 hover:border-ds-gray-500",
            "transition-colors duration-150 shadow-sm",
          ].join(" ")}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          {collapsed ? <ChevronRight size={12} /> : <ChevronLeft size={12} />}
        </button>
      </aside>
    </>
  );
}
