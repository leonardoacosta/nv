"use client";

import { useEffect, useState } from "react";
import { Brain, AlertCircle, RefreshCw, Search, FileText } from "lucide-react";
import MemoryPreview, { type MemoryFile } from "@/components/MemoryPreview";
import type { MemoryListResponse } from "@/types/api";

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
      const raw = (await res.json()) as MemoryListResponse;

      let parsed: MemoryFile[] = [];
      if (Array.isArray(raw.topics)) {
        // Backend returns { topics: string[] } — map each topic name to a MemoryFile.
        // Content is empty until a specific topic is fetched via ?topic=<name>.
        parsed = raw.topics.map((topic) => ({
          name: topic,
          path: topic,
          content: "",
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleSave = async (path: string, content: string): Promise<void> => {
    const res = await fetch("/api/memory", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ topic: path, content }),
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
          <h1 className="text-2xl font-semibold text-ds-gray-1000">Memory</h1>
          <p className="mt-1 text-sm text-ds-gray-900">
            {loading ? "Loading..." : `${files.length} memory files`}
          </p>
        </div>
        <button
          type="button"
          onClick={() => void fetchMemory()}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-sm text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {error && (
        <div className="flex items-center gap-3 p-4 mb-4 rounded-xl bg-red-700/10 border border-red-700/30 text-red-700">
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
              className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-900"
            />
            <input
              type="text"
              placeholder="Search memory..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-full pl-9 pr-4 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-sm text-ds-gray-1000 placeholder:text-ds-gray-900 focus:outline-none focus:border-ds-gray-1000/60 transition-colors"
            />
          </div>

          <div className="flex-1 overflow-y-auto space-y-1 pr-1">
            {loading ? (
              Array.from({ length: 6 }).map((_, i) => (
                <div
                  key={i}
                  className="h-12 animate-pulse rounded-lg bg-ds-gray-100 border border-ds-gray-400"
                />
              ))
            ) : filtered.length === 0 ? (
              <div className="flex flex-col items-center gap-3 py-12 text-ds-gray-900">
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
                      ? "bg-ds-gray-alpha-200 border border-ds-gray-1000/40"
                      : "hover:bg-ds-gray-100 border border-transparent hover:border-ds-gray-400"
                  }`}
                >
                  <FileText
                    size={14}
                    className={
                      selected?.path === file.path
                        ? "text-ds-gray-1000 shrink-0 mt-0.5"
                        : "text-ds-gray-900 shrink-0 mt-0.5"
                    }
                  />
                  <div className="min-w-0">
                    <p className="text-xs font-medium text-ds-gray-1000 truncate">
                      {file.name}
                    </p>
                    {file.topics && file.topics.length > 0 && (
                      <p className="text-xs text-ds-gray-900 truncate mt-0.5">
                        {file.topics.join(", ")}
                      </p>
                    )}
                    {file.size_bytes !== undefined && (
                      <p className="text-xs text-ds-gray-900 font-mono">
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
