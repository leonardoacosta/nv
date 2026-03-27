"use client";

import { useCallback, useEffect, useState } from "react";
import { Loader2, X } from "lucide-react";
import type {
  CreateProjectRequest,
  ProjectCategory,
  ProjectEntity,
} from "@/types/api";
import { apiFetch } from "@/lib/api-client";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const CATEGORIES: { value: ProjectCategory; label: string }[] = [
  { value: "work", label: "Work" },
  { value: "personal", label: "Personal" },
  { value: "open_source", label: "Open Source" },
  { value: "archived", label: "Archived" },
];

const KEBAB_CASE_RE = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface CreateProjectDialogProps {
  open: boolean;
  onClose: () => void;
  onCreated: (project: ProjectEntity) => void;
}

export default function CreateProjectDialog({
  open,
  onClose,
  onCreated,
}: CreateProjectDialogProps) {
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const [category, setCategory] = useState<ProjectCategory>("work");
  const [path, setPath] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reset form on open
  useEffect(() => {
    if (open) {
      setCode("");
      setName("");
      setCategory("work");
      setPath("");
      setError(null);
    }
  }, [open]);

  // Close on Escape
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    },
    [onClose],
  );

  useEffect(() => {
    if (!open) return;
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [open, handleKeyDown]);

  if (!open) return null;

  const codeValid = code.length > 0 && KEBAB_CASE_RE.test(code);
  const nameValid = name.trim().length > 0;
  const canSubmit = codeValid && nameValid && !submitting;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSubmit) return;

    setSubmitting(true);
    setError(null);
    try {
      const body: CreateProjectRequest = {
        code,
        name: name.trim(),
        category,
      };
      if (path.trim()) body.path = path.trim();

      const res = await apiFetch("/api/projects", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      });

      if (!res.ok) {
        const data = (await res.json().catch(() => null)) as {
          error?: string;
        } | null;
        throw new Error(data?.error ?? `HTTP ${res.status}`);
      }

      const project = (await res.json()) as ProjectEntity;
      onCreated(project);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create project");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/40 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Dialog */}
      <div className="relative z-10 w-full max-w-md mx-4 bg-ds-bg-100 border border-ds-gray-400 rounded-xl shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-ds-gray-400">
          <h2 className="text-heading-16 text-ds-gray-1000">
            Create Project
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="flex items-center justify-center size-8 rounded-lg text-ds-gray-900 hover:text-ds-gray-1000 hover:bg-ds-gray-100/40 transition-colors"
            aria-label="Close dialog"
          >
            <X size={16} />
          </button>
        </div>

        {/* Form */}
        <form onSubmit={(e) => void handleSubmit(e)} className="p-5 flex flex-col gap-4">
          {/* Code */}
          <div className="flex flex-col gap-1">
            <label htmlFor="project-code" className="text-label-12 text-ds-gray-900">
              Code
            </label>
            <input
              id="project-code"
              type="text"
              value={code}
              onChange={(e) => setCode(e.target.value.toLowerCase())}
              placeholder="my-project"
              className="px-3 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-14 text-ds-gray-1000 font-mono placeholder:text-ds-gray-700 focus:outline-none focus:border-ds-gray-1000/60 transition-colors"
              autoFocus
            />
            {code.length > 0 && !codeValid && (
              <p className="text-copy-13 text-red-700">
                Must be kebab-case (lowercase letters, numbers, hyphens)
              </p>
            )}
          </div>

          {/* Name */}
          <div className="flex flex-col gap-1">
            <label htmlFor="project-name" className="text-label-12 text-ds-gray-900">
              Name
            </label>
            <input
              id="project-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My Project"
              className="px-3 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-14 text-ds-gray-1000 placeholder:text-ds-gray-700 focus:outline-none focus:border-ds-gray-1000/60 transition-colors"
            />
          </div>

          {/* Category */}
          <div className="flex flex-col gap-1">
            <label htmlFor="project-category" className="text-label-12 text-ds-gray-900">
              Category
            </label>
            <select
              id="project-category"
              value={category}
              onChange={(e) => setCategory(e.target.value as ProjectCategory)}
              className="px-3 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-13 text-ds-gray-1000 focus:outline-none focus:border-ds-gray-1000/60 transition-colors"
            >
              {CATEGORIES.map((c) => (
                <option key={c.value} value={c.value}>
                  {c.label}
                </option>
              ))}
            </select>
          </div>

          {/* Path (optional) */}
          <div className="flex flex-col gap-1">
            <label htmlFor="project-path" className="text-label-12 text-ds-gray-900">
              Path (optional)
            </label>
            <input
              id="project-path"
              type="text"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="~/dev/my-project"
              className="px-3 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-14 text-ds-gray-1000 font-mono placeholder:text-ds-gray-700 focus:outline-none focus:border-ds-gray-1000/60 transition-colors"
            />
          </div>

          {/* Error */}
          {error && (
            <p className="text-copy-13 text-red-700">{error}</p>
          )}

          {/* Submit */}
          <button
            type="submit"
            disabled={!canSubmit}
            className="flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg text-button-14 font-medium bg-ds-gray-1000 text-ds-bg-100 hover:bg-ds-gray-900 transition-colors disabled:opacity-50"
          >
            {submitting && <Loader2 size={14} className="animate-spin" />}
            Create Project
          </button>
        </form>
      </div>
    </div>
  );
}
