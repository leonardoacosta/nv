"use client";

import { usePathname } from "next/navigation";
import Sidebar from "@/components/Sidebar";
import { DaemonEventProvider } from "@/components/providers/DaemonEventContext";
import { TRPCReactProvider } from "@/lib/trpc/react";
import QueryInvalidationBridge from "@/components/providers/QueryInvalidationBridge";

export default function AppShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const isLoginPage = pathname === "/login";

  if (isLoginPage) {
    return <>{children}</>;
  }

  return (
    <TRPCReactProvider>
      <DaemonEventProvider>
        <QueryInvalidationBridge />
        <div className="flex h-dvh overflow-hidden bg-ds-bg-100">
          <Sidebar />
          <main className="flex-1 overflow-auto pt-16 sm:pt-0">{children}</main>
        </div>
      </DaemonEventProvider>
    </TRPCReactProvider>
  );
}
