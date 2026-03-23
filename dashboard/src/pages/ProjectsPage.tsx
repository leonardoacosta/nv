import { useEffect, useState } from "react";
import { FolderOpen, AlertCircle, RefreshCw, Search } from "lucide-react";
import ProjectAccordion, {
  type Project,
} from "@/components/ProjectAccordion";

export default function ProjectsPage() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  const fetchProjects = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/projects");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as Project[];
      setProjects(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load projects");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void fetchProjects();
  }, []);

  const handleSolveWithNexus = (projectId: string, errorId: string) => {
    console.log("Solve with Nexus:", projectId, errorId);
    // Future: navigate to Nexus with context pre-filled
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
    <div className="p-8 space-y-6 max-w-4xl">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-cosmic-bright">
            Projects
          </h1>
          <p className="mt-1 text-sm text-cosmic-muted">
            {loading
              ? "Loading..."
              : `${projects.length} projects · ${errorCount} issues`}
          </p>
        </div>
        <button
          type="button"
          onClick={() => void fetchProjects()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {/* Search */}
      <div className="relative">
        <Search
          size={14}
          className="absolute left-3 top-1/2 -translate-y-1/2 text-cosmic-muted"
        />
        <input
          type="text"
          placeholder="Search projects..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="w-full pl-9 pr-4 py-2 rounded-lg bg-cosmic-surface border border-cosmic-border text-sm text-cosmic-text placeholder:text-cosmic-muted focus:outline-none focus:border-cosmic-purple/60 transition-colors"
        />
      </div>

      {error && (
        <div className="flex items-center gap-3 p-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose">
          <AlertCircle size={16} />
          <span className="text-sm">{error}</span>
        </div>
      )}

      {loading ? (
        <div className="space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <div
              key={i}
              className="h-14 animate-pulse rounded-cosmic bg-cosmic-surface border border-cosmic-border"
            />
          ))}
        </div>
      ) : filtered.length === 0 ? (
        <div className="flex flex-col items-center gap-3 py-16 text-cosmic-muted">
          <FolderOpen size={36} />
          <p className="text-sm">
            {search ? "No projects match your search" : "No projects found"}
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {filtered.map((project) => (
            <ProjectAccordion
              key={project.id}
              project={project}
              onSolveWithNexus={handleSolveWithNexus}
            />
          ))}
        </div>
      )}
    </div>
  );
}
