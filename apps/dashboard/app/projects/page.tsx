"use client";

import { useEffect, useState } from "react";
import { FolderOpen, RefreshCw, Search } from "lucide-react";
import ProjectAccordion, {
  type Project,
  type ProjectError,
} from "@/components/ProjectAccordion";
import ErrorBanner from "@/components/layout/ErrorBanner";
import EmptyState from "@/components/layout/EmptyState";
import type { ApiProject, ProjectsGetResponse } from "@/types/api";
import { apiFetch } from "@/lib/api-client";

/** Map daemon ApiProject ({ code, path }) to the component Project interface. */
function mapApiProject(p: ApiProject): Project {
  return {
    id: p.code,
    name: p.code,
    path: p.path,
    status: "unknown",
    errors: [],
  };
}

export default function ProjectsPage() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [solveStatus, setSolveStatus] = useState<string | null>(null);

  const fetchProjects = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await apiFetch("/api/projects");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as ProjectsGetResponse;
      setProjects((data.projects ?? []).map(mapApiProject));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load projects");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void fetchProjects();
  }, []);

  const handleSolveWithNexus = async (projectId: string, errorId: string) => {
    const project = projects.find((p) => p.id === projectId);
    const err = project?.errors?.find((e: ProjectError) => e.id === errorId);
    if (!project || !err) return;

    setSolveStatus("Starting Nexus session...");
    try {
      const res = await apiFetch("/api/solve", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          project: project.id,
          error: err.message,
          context: err.file ? `${err.file}${err.line ? `:${err.line}` : ""}` : undefined,
        }),
      });
      if (!res.ok) {
        const data = (await res.json()) as { error?: string };
        setSolveStatus(data.error ?? `HTTP ${res.status}`);
      } else {
        const data = (await res.json()) as { session_id: string };
        setSolveStatus(`Session started: ${data.session_id.slice(0, 8)}...`);
      }
    } catch (e) {
      setSolveStatus(e instanceof Error ? e.message : "Failed to start session");
    } finally {
      setTimeout(() => setSolveStatus(null), 4000);
    }
  };

  const filtered = projects.filter(
    (p) =>
      search === "" ||
      p.name.toLowerCase().includes(search.toLowerCase()) ||
      p.path?.toLowerCase().includes(search.toLowerCase())
  );

  const errorCount = projects.reduce(
    (acc, p) => acc + (p.errors?.length ?? 0),
    0
  );

  return (
    <div className="p-8 space-y-6 max-w-4xl animate-fade-in-up">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-heading-24 text-ds-gray-1000">
            Projects
          </h1>
          <p className="mt-1 text-copy-14 text-ds-gray-900">
            {loading
              ? "Loading..."
              : `${projects.length} projects · ${errorCount} issues`}
          </p>
        </div>
        <button
          type="button"
          onClick={() => void fetchProjects()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-button-14 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {/* Search */}
      <div className="relative">
        <Search
          size={14}
          className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-900"
        />
        <input
          type="text"
          placeholder="Search projects..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="w-full pl-9 pr-4 py-2 surface-inset text-label-14 text-ds-gray-1000 placeholder:text-ds-gray-900 focus:outline-none focus:border-ds-gray-1000/60 transition-colors"
        />
      </div>

      {solveStatus && (
        <div className="flex items-center gap-3 p-3 rounded-xl bg-ds-gray-alpha-100 border border-ds-gray-1000/30 text-ds-gray-1000">
          <span className="text-sm">{solveStatus}</span>
        </div>
      )}

      {error && (
        <ErrorBanner
          message="Failed to load projects"
          detail={error}
          onRetry={() => void fetchProjects()}
        />
      )}

      {loading ? (
        <div className="space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <div
              key={i}
              className="h-14 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
            />
          ))}
        </div>
      ) : filtered.length === 0 ? (
        <EmptyState
          title={search ? "No projects match your search" : "No projects found"}
          description={
            search
              ? "Try a different search term."
              : "Projects registered with the daemon will appear here."
          }
          icon={<FolderOpen size={40} aria-hidden="true" />}
        />
      ) : (
        <div className="space-y-2">
          {filtered.map((project, idx) => (
            <div
              key={project.id}
              className={`animate-fade-in-up ${idx < 10 ? `stagger-${Math.min(idx + 1, 10)}` : ""}`}
            >
              <ProjectAccordion
                project={project}
                onSolveWithNexus={handleSolveWithNexus}
              />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
