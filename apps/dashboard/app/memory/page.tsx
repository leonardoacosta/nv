"use client";

import { useEffect, useState } from "react";
import { Brain, AlertCircle, RefreshCw, Search, FileText } from "lucide-react";
import MemoryPreview, { type MemoryFile } from "@/components/MemoryPreview";
import PageShell from "@/components/layout/PageShell";
import ErrorBanner from "@/components/layout/ErrorBanner";
import EmptyState from "@/components/layout/EmptyState";
import type { MemoryListResponse, MemoryTopicResponse } from "@/types/api";

export default function MemoryPage() {
  const [files, setFiles] = useState<MemoryFile[]>([]);
  const [selected, setSelected] = useState<MemoryFile | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  const fetchTopicContent = async (topic: string): Promise<string> => {
    try {
      const res = await fetch(`/api/memory?topic=${encodeURIComponent(topic)}`);
      if (!res.ok) return "";
      const data = (await res.json()) as MemoryTopicResponse;
      return data.content ?? "";
    } catch {
      return "";
    }
  };

  const fetchMemory = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/memory");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const raw = (await res.json()) as MemoryListResponse;

      let parsed: MemoryFile[] = [];
      if (Array.isArray(raw.topics)) {
        parsed = raw.topics.map((topic) => ({
          name: topic,
          path: topic,
          content: "",
        }));
      }

      setFiles(parsed);
      if (parsed.length > 0 && !selected && parsed[0]) {
        // Auto-select first file and fetch its content
        const first = parsed[0];
        setSelected(first);
        const content = await fetchTopicContent(first.path);
        const updated = { ...first, content };
        setSelected(updated);
        setFiles((prev) =>
          prev.map((f) => (f.path === first.path ? updated : f)),
        );
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load memory");
    } finally {
      setLoading(false);
    }
  };

  const handleSelect = async (file: MemoryFile) => {
    setSelected(file);
    // Lazy-load content if not yet fetched
    if (!file.content) {
      const content = await fetchTopicContent(file.path);
      const updated = { ...file, content };
      setSelected(updated);
      setFiles((prev) =>
        prev.map((f) => (f.path === file.path ? updated : f)),
      );
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

    setFiles((prev) =>
      prev.map((f) => (f.path === path ? { ...f, content } : f)),
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
        t.toLowerCase().includes(search.toLowerCase()),
      ),
  );

  const headerAction = (
    <button
      type="button"
      onClick={() => void fetchMemory()}
      disabled={loading}
      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-label-13 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
    >
      <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
      Refresh
    </button>
  );

  return (
    <PageShell
      title="Memory"
      subtitle={loading ? "Loading..." : `${files.length} memory files`}
      action={headerAction}
    >
      <div className="animate-fade-in-up space-y-4 h-[calc(100vh-160px)]">
        {error && (
          <ErrorBanner
            message="Failed to load memory"
            detail={error}
            onRetry={() => void fetchMemory()}
          />
        )}

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-4 h-full">
          {/* File list panel */}
          <div className="flex flex-col gap-3 overflow-hidden">
            {/* Search input — surface-inset */}
            <div className="relative shrink-0">
              <Search
                size={14}
                className="absolute left-3 top-1/2 -translate-y-1/2 text-ds-gray-700 pointer-events-none"
              />
              <input
                type="text"
                placeholder="Search memory..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="w-full pl-9 pr-4 py-2 surface-inset text-label-13 text-ds-gray-1000 placeholder:text-ds-gray-700 focus:outline-none focus:border-ds-gray-500 transition-colors"
              />
            </div>

            {/* File list */}
            <div className="flex-1 overflow-y-auto space-y-1.5 pr-1">
              {loading ? (
                Array.from({ length: 6 }).map((_, i) => (
                  <div
                    key={i}
                    className="h-12 animate-pulse rounded-xl bg-ds-gray-100 border border-ds-gray-alpha-400"
                  />
                ))
              ) : filtered.length === 0 ? (
                <EmptyState
                  title={search ? "No files match" : "No memory files"}
                  description={
                    search
                      ? "Try a different search term."
                      : "Memory files will appear here once created."
                  }
                  icon={<Brain size={24} aria-hidden="true" />}
                />
              ) : (
                filtered.map((file) => (
                  <button
                    key={file.path}
                    type="button"
                    onClick={() => void handleSelect(file)}
                    className={[
                      "w-full flex items-start gap-3 px-3 py-2.5 rounded-xl text-left transition-colors",
                      selected?.path === file.path
                        ? "surface-raised border-ds-gray-1000/40"
                        : "surface-base hover:border-ds-gray-500",
                    ].join(" ")}
                  >
                    <FileText
                      size={14}
                      className={
                        selected?.path === file.path
                          ? "text-ds-gray-1000 shrink-0 mt-0.5"
                          : "text-ds-gray-700 shrink-0 mt-0.5"
                      }
                    />
                    <div className="min-w-0 flex-1">
                      <p className="text-label-13 text-ds-gray-1000 truncate font-medium">
                        {file.name}
                      </p>
                      {file.topics && file.topics.length > 0 && (
                        <p className="text-copy-13 text-ds-gray-900 truncate mt-0.5">
                          {file.topics.join(", ")}
                        </p>
                      )}
                      <div className="flex items-center gap-2 mt-0.5">
                        {file.content && (
                          <span className="text-label-13-mono text-ds-gray-700">
                            {file.content.split(/\s+/).filter(Boolean).length}w
                          </span>
                        )}
                        {file.size_bytes !== undefined && (
                          <span className="text-label-13-mono text-ds-gray-700">
                            {(file.size_bytes / 1024).toFixed(1)} KB
                          </span>
                        )}
                        {file.updated_at && (
                          <span
                            className="text-label-13-mono text-ds-gray-700"
                            suppressHydrationWarning
                          >
                            {new Date(file.updated_at).toLocaleDateString()}
                          </span>
                        )}
                      </div>
                    </div>
                  </button>
                ))
              )}
            </div>
          </div>

          {/* Preview panel */}
          <div className="lg:col-span-2 min-h-0">
            <MemoryPreview file={selected} onSave={handleSave} />
          </div>
        </div>
      </div>
    </PageShell>
  );
}
