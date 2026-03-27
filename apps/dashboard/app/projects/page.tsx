"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { FolderOpen, Plus, RefreshCw, Search } from "lucide-react";
import PageShell from "@/components/layout/PageShell";
import ErrorBanner from "@/components/layout/ErrorBanner";
import ProjectCategoryTree from "@/components/ProjectCategoryTree";
import ProjectDetailPanel from "@/components/ProjectDetailPanel";
import CreateProjectDialog from "@/components/CreateProjectDialog";
import type {
  ProjectCategory,
  ProjectEntity,
  ProjectsListResponse,
  UpdateProjectRequest,
} from "@/types/api";
import { apiFetch } from "@/lib/api-client";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type FilterTab = "all" | "work" | "personal" | "open_source" | "archived";

const FILTER_TABS: { key: FilterTab; label: string }[] = [
  { key: "all", label: "All" },
  { key: "work", label: "Work" },
  { key: "personal", label: "Personal" },
  { key: "open_source", label: "Open Source" },
  { key: "archived", label: "Archived" },
];

// ---------------------------------------------------------------------------
// ProjectsPage
// ---------------------------------------------------------------------------

export default function ProjectsPage() {
  // 1. Local State
  const [projects, setProjects] = useState<ProjectEntity[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [filterTab, setFilterTab] = useState<FilterTab>("all");
  const [selectedProject, setSelectedProject] = useState<ProjectEntity | null>(
    null,
  );
  const [createOpen, setCreateOpen] = useState(false);
  const [refreshing, setRefreshing] = useState(false);

  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 2. Fetch projects
  const fetchProjects = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await apiFetch("/api/projects");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as ProjectsListResponse;
      setProjects(data.projects ?? []);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load projects",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  // 3. Initial load
  useEffect(() => {
    void fetchProjects();
  }, [fetchProjects]);

  // 4. Search debounce
  const handleSearchChange = (value: string) => {
    setSearch(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      setDebouncedSearch(value);
    }, 300);
  };

  // 5. Refresh handler — extract then re-fetch
  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      await apiFetch("/api/projects/extract", { method: "POST" });
      await fetchProjects();
    } catch {
      // fetchProjects handles its own error
    } finally {
      setRefreshing(false);
    }
  };

  // 6. Update handler
  const handleUpdate = async (code: string, data: UpdateProjectRequest) => {
    try {
      const res = await apiFetch(`/api/projects/${code}`, {
        method: "PUT",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(data),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const updated = (await res.json()) as ProjectEntity;

      // Update local state
      setProjects((prev) =>
        prev.map((p) => (p.code === code ? updated : p)),
      );
      setSelectedProject(updated);
    } catch {
      // Error handled in detail panel
    }
  };

  // 7. Select handler
  const handleSelectProject = (code: string) => {
    const project = projects.find((p) => p.code === code) ?? null;
    setSelectedProject(project);
  };

  // 8. Derived values
  const filtered = projects.filter((p) => {
    // Filter by tab
    if (filterTab !== "all" && p.category !== filterTab) return false;

    // Filter by search
    if (debouncedSearch) {
      const q = debouncedSearch.toLowerCase();
      const matchesName = p.name.toLowerCase().includes(q);
      const matchesCode = p.code.toLowerCase().includes(q);
      const matchesDesc = p.description?.toLowerCase().includes(q) ?? false;
      if (!matchesName && !matchesCode && !matchesDesc) return false;
    }

    return true;
  });

  const distinctCategories = new Set(projects.map((p) => p.category));

  // 9. Header action
  const headerAction = (
    <div className="flex items-center gap-2">
      <button
        type="button"
        onClick={() => void handleRefresh()}
        disabled={refreshing || loading}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
      >
        <RefreshCw
          size={12}
          className={refreshing ? "animate-spin" : ""}
        />
        Refresh
      </button>
      <button
        type="button"
        onClick={() => setCreateOpen(true)}
        className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-1000 bg-ds-gray-alpha-200 hover:bg-ds-gray-700/30 transition-colors"
      >
        <Plus size={12} />
        Create
      </button>
    </div>
  );

  // 10. Render
  return (
    <>
      <PageShell
        title="Projects"
        subtitle={
          loading
            ? "Loading..."
            : `${projects.length} projects across ${distinctCategories.size} categories`
        }
        action={headerAction}
      >
        <div className="flex flex-col gap-3">
          {/* Filter tabs */}
          <div className="flex items-center gap-1 overflow-x-auto">
            {FILTER_TABS.map((tab) => (
              <button
                key={tab.key}
                type="button"
                onClick={() => setFilterTab(tab.key)}
                className={[
                  "px-3 py-1.5 rounded-full text-label-13 whitespace-nowrap transition-colors",
                  filterTab === tab.key
                    ? "bg-ds-gray-alpha-200 text-ds-gray-1000"
                    : "text-ds-gray-900 hover:text-ds-gray-1000 hover:bg-ds-gray-100/40",
                ].join(" ")}
              >
                {tab.label}
              </button>
            ))}
          </div>

          {/* Search */}
          <div className="relative max-w-sm">
            <Search
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-900 pointer-events-none"
            />
            <input
              type="text"
              value={search}
              onChange={(e) => handleSearchChange(e.target.value)}
              placeholder="Search projects..."
              className="w-full pl-9 pr-4 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-13 text-ds-gray-1000 placeholder:text-ds-gray-900 focus:outline-hidden focus:border-ds-gray-1000/60 transition-colors"
            />
          </div>

          {error && (
            <ErrorBanner
              message="Failed to load projects"
              detail={error}
              onRetry={() => void fetchProjects()}
            />
          )}

          {/* Loading skeleton */}
          {loading ? (
            <div className="flex flex-col gap-2">
              {Array.from({ length: 5 }).map((_, i) => (
                <div
                  key={i}
                  className="h-20 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-400"
                />
              ))}
            </div>
          ) : filtered.length === 0 ? (
            /* Empty state */
            <div className="flex flex-col items-center gap-3 py-16">
              <FolderOpen size={28} className="text-ds-gray-600" />
              <p className="text-copy-13 text-ds-gray-900 text-center">
                {debouncedSearch || filterTab !== "all"
                  ? "No projects match your filters"
                  : "No projects found"}
              </p>
            </div>
          ) : (
            /* Project tree */
            <ProjectCategoryTree
              projects={filtered}
              onSelectProject={handleSelectProject}
            />
          )}
        </div>
      </PageShell>

      {/* Detail panel */}
      {selectedProject && (
        <ProjectDetailPanel
          project={selectedProject}
          onClose={() => setSelectedProject(null)}
          onUpdate={(code, data) => void handleUpdate(code, data)}
        />
      )}

      {/* Create dialog */}
      <CreateProjectDialog
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        onCreated={(project) => {
          setProjects((prev) => [...prev, project]);
        }}
      />
    </>
  );
}
