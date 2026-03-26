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
  Monitor,
  Activity,
  Layers,
  MessageSquare,
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
// WebSocket status indicator
// ---------------------------------------------------------------------------

const WS_STATUS_CONFIG: Record<
  WsStatus,
  { dot: string; label: string; text: string }
> = {
  connected: {
    dot: "bg-emerald-400",
    label: "Daemon connected",
    text: "Connected",
  },
  reconnecting: {
    dot: "bg-amber-400 animate-pulse",
    label: "Daemon reconnecting",
    text: "Reconnecting…",
  },
  disconnected: {
    dot: "bg-red-400",
    label: "Daemon disconnected",
    text: "Disconnected",
  },
};

function WsStatusDot({ collapsed }: { collapsed: boolean }) {
  const status = useDaemonStatus();
  const cfg = WS_STATUS_CONFIG[status];
  return (
    <div
      className="flex items-center gap-2 px-3 py-2 min-h-11"
      title={cfg.label}
    >
      <span
        className={`inline-block w-2 h-2 rounded-full shrink-0 ${cfg.dot}`}
        aria-label={cfg.label}
        role="img"
      />
      {!collapsed && (
        <span className="text-xs text-cosmic-muted truncate">{cfg.text}</span>
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

  // Subscribe to approval.created events for real-time badge updates
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
// Nav items
// ---------------------------------------------------------------------------

interface NavItem {
  to: string;
  label: string;
  icon: React.ElementType;
  badge?: "approvals";
}

const NAV_ITEMS: NavItem[] = [
  { to: "/", label: "Dashboard", icon: LayoutDashboard },
  { to: "/briefing", label: "Briefing", icon: Sun },
  { to: "/obligations", label: "Obligations", icon: CheckSquare },
  { to: "/approvals", label: "Approvals", icon: ShieldAlert, badge: "approvals" },
  { to: "/diary", label: "Diary", icon: BookOpen },
  { to: "/sessions", label: "Sessions", icon: Layers },
  { to: "/messages", label: "Messages", icon: MessageSquare },
  { to: "/projects", label: "Projects", icon: FolderOpen },
  { to: "/integrations", label: "Integrations", icon: Plug },
  { to: "/usage", label: "Usage", icon: BarChart3 },
  { to: "/cold-starts", label: "Cold Starts", icon: Activity },
  { to: "/memory", label: "Memory", icon: Brain },
  { to: "/session", label: "CC Session", icon: Monitor },
  { to: "/settings", label: "Settings", icon: Settings },
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
        "flex items-center gap-3 px-3 py-2 min-h-11 rounded-lg text-sm font-medium",
        "transition-colors duration-150 group relative",
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
      {!collapsed && <span className="truncate flex-1">{label}</span>}

      {/* Badge */}
      {badgeCount > 0 && (
        <span
          className={[
            "inline-flex items-center justify-center rounded-full text-xs font-mono font-bold",
            "bg-amber-500/20 text-amber-400 border border-amber-500/30",
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
      {/* Nova mark / logo */}
      <div className="flex flex-col border-b border-cosmic-border overflow-hidden shrink-0">
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
        {NAV_ITEMS.map((item) => {
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
      </nav>

      {/* Footer: WebSocket status */}
      <div className="border-t border-cosmic-border shrink-0">
        <WsStatusDot collapsed={collapsed} />
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

  // Trap focus / close on outside click for mobile drawer
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
          "flex items-center justify-center w-11 h-11 rounded-lg",
          "bg-cosmic-dark border border-cosmic-border text-cosmic-muted",
          "hover:text-cosmic-text hover:border-cosmic-purple/50 transition-colors",
          "shadow-cosmic-sm",
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
            className="relative z-10 flex flex-col w-64 bg-cosmic-dark border-r border-cosmic-border"
          >
            {/* Close button */}
            <button
              type="button"
              onClick={() => setMobileOpen(false)}
              aria-label="Close navigation"
              className="absolute top-3 right-3 flex items-center justify-center w-9 h-9 rounded-lg text-cosmic-muted hover:text-cosmic-text hover:bg-cosmic-surface transition-colors"
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
        className="hidden sm:flex md:hidden relative flex-col bg-cosmic-dark border-r border-cosmic-border w-16 shrink-0"
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
          "hidden md:relative md:flex flex-col bg-cosmic-dark border-r border-cosmic-border",
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
            "bg-cosmic-surface border border-cosmic-border",
            "text-cosmic-muted hover:text-cosmic-text hover:border-cosmic-purple",
            "transition-colors duration-150 shadow-cosmic-sm",
          ].join(" ")}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          {collapsed ? <ChevronRight size={12} /> : <ChevronLeft size={12} />}
        </button>
      </aside>
    </>
  );
}
