import { useEffect, useState } from "react";
import { Brain, AlertCircle, RefreshCw, Search, FileText } from "lucide-react";
import MemoryPreview, { type MemoryFile } from "@/components/MemoryPreview";

interface MemoryApiResponse {
  files?: MemoryFile[];
  topics?: string[];
  [key: string]: unknown;
}

export default function MemoryPage() {
  const [files, setFiles] = useState<MemoryFile[]>([]);
  const [selected, setSelected] = useState<MemoryFile | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  const fetchMemory = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/memory");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const raw = (await res.json()) as MemoryApiResponse | MemoryFile[];

      let parsed: MemoryFile[] = [];
      if (Array.isArray(raw)) {
        parsed = raw as MemoryFile[];
      } else if (raw.files && Array.isArray(raw.files)) {
        parsed = raw.files;
      } else {
        // Treat object keys as file entries
        parsed = Object.entries(raw).map(([name, content]) => ({
          name,
          path: name,
          content: typeof content === "string" ? content : JSON.stringify(content, null, 2),
        }));
      }

      setFiles(parsed);
      if (parsed.length > 0 && !selected) {
        setSelected(parsed[0] ?? null);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load memory");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void fetchMemory();
  }, []);

  const handleSave = async (path: string, content: string): Promise<void> => {
    const res = await fetch("/api/memory", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ path, content }),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);

    // Update local state
    setFiles((prev) =>
      prev.map((f) => (f.path === path ? { ...f, content } : f))
    );
    if (selected?.path === path) {
      setSelected((prev) => (prev ? { ...prev, content } : prev));
    }
  };

  const filtered = files.filter(
    (f) =>
      search === "" ||
      f.name.toLowerCase().includes(search.toLowerCase()) ||
      (f.topics ?? []).some((t) =>
        t.toLowerCase().includes(search.toLowerCase())
      )
  );

  return (
    <div className="p-8 h-full max-w-7xl">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-semibold text-cosmic-bright">Memory</h1>
          <p className="mt-1 text-sm text-cosmic-muted">
            {loading ? "Loading..." : `${files.length} memory files`}
          </p>
        </div>
        <button
          type="button"
          onClick={() => void fetchMemory()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-cosmic-muted hover:text-cosmic-text border border-cosmic-border hover:border-cosmic-purple/50 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {error && (
        <div className="flex items-center gap-3 p-4 mb-4 rounded-cosmic bg-cosmic-rose/10 border border-cosmic-rose/30 text-cosmic-rose">
          <AlertCircle size={16} />
          <span className="text-sm">{error}</span>
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4 h-[calc(100vh-220px)]">
        {/* File list */}
        <div className="flex flex-col gap-3 overflow-hidden">
          <div className="relative shrink-0">
            <Search
              size={14}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-cosmic-muted"
            />
            <input
              type="text"
              placeholder="Search memory..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-full pl-9 pr-4 py-2 rounded-lg bg-cosmic-surface border border-cosmic-border text-sm text-cosmic-text placeholder:text-cosmic-muted focus:outline-none focus:border-cosmic-purple/60 transition-colors"
            />
          </div>

          <div className="flex-1 overflow-y-auto space-y-1 pr-1">
            {loading ? (
              Array.from({ length: 6 }).map((_, i) => (
                <div
                  key={i}
                  className="h-12 animate-pulse rounded-lg bg-cosmic-surface border border-cosmic-border"
                />
              ))
            ) : filtered.length === 0 ? (
              <div className="flex flex-col items-center gap-3 py-12 text-cosmic-muted">
                <Brain size={28} />
                <p className="text-xs">
                  {search ? "No files match" : "No memory files found"}
                </p>
              </div>
            ) : (
              filtered.map((file) => (
                <button
                  key={file.path}
                  type="button"
                  onClick={() => setSelected(file)}
                  className={`w-full flex items-start gap-3 px-3 py-2.5 rounded-lg text-left transition-colors ${
                    selected?.path === file.path
                      ? "bg-cosmic-purple/20 border border-cosmic-purple/40"
                      : "hover:bg-cosmic-surface border border-transparent hover:border-cosmic-border"
                  }`}
                >
                  <FileText
                    size={14}
                    className={
                      selected?.path === file.path
                        ? "text-cosmic-purple shrink-0 mt-0.5"
                        : "text-cosmic-muted shrink-0 mt-0.5"
                    }
                  />
                  <div className="min-w-0">
                    <p className="text-xs font-medium text-cosmic-text truncate">
                      {file.name}
                    </p>
                    {file.topics && file.topics.length > 0 && (
                      <p className="text-xs text-cosmic-muted truncate mt-0.5">
                        {file.topics.join(", ")}
                      </p>
                    )}
                    {file.size_bytes !== undefined && (
                      <p className="text-xs text-cosmic-muted font-mono">
                        {(file.size_bytes / 1024).toFixed(1)} KB
                      </p>
                    )}
                  </div>
                </button>
              ))
            )}
          </div>
        </div>

        {/* Preview panel */}
        <div className="lg:col-span-2 min-h-0">
          <MemoryPreview
            file={selected}
            onSave={handleSave}
          />
        </div>
      </div>
    </div>
  );
}
