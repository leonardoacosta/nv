import type { Metadata } from "next";
import "./globals.css";
import Sidebar from "@/components/Sidebar";
import { DaemonEventProvider } from "@/components/providers/DaemonEventContext";

export const metadata: Metadata = {
  title: "Nova Dashboard",
  description: "Nova orchestration dashboard",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>
        <DaemonEventProvider>
          <div className="flex min-h-dvh bg-ds-bg-100">
            <Sidebar />
            <main className="flex-1 overflow-auto pt-16 sm:pt-0">{children}</main>
          </div>
        </DaemonEventProvider>
      </body>
    </html>
  );
}
