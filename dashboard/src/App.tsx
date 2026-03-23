import { BrowserRouter, Routes, Route } from "react-router-dom";
import Sidebar from "@/components/Sidebar";
import DashboardPage from "@/pages/DashboardPage";
import ObligationsPage from "@/pages/ObligationsPage";
import ProjectsPage from "@/pages/ProjectsPage";
import NexusPage from "@/pages/NexusPage";
import IntegrationsPage from "@/pages/IntegrationsPage";
import UsagePage from "@/pages/UsagePage";
import MemoryPage from "@/pages/MemoryPage";
import SettingsPage from "@/pages/SettingsPage";

export default function App() {
  return (
    <BrowserRouter>
      <div className="flex min-h-dvh bg-cosmic-gradient">
        <Sidebar />
        <main className="flex-1 overflow-auto">
          <Routes>
            <Route path="/" element={<DashboardPage />} />
            <Route path="/obligations" element={<ObligationsPage />} />
            <Route path="/projects" element={<ProjectsPage />} />
            <Route path="/nexus" element={<NexusPage />} />
            <Route path="/integrations" element={<IntegrationsPage />} />
            <Route path="/usage" element={<UsagePage />} />
            <Route path="/memory" element={<MemoryPage />} />
            <Route path="/settings" element={<SettingsPage />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
}
