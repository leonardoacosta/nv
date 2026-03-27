"use client";

import { useCallback, useEffect, useState } from "react";
import {
  CheckSquare,
  Layers,
  Loader2,
  MessageSquare,
  RefreshCw,
  Save,
  X,
} from "lucide-react";
import type {
  ProjectCategory,
  ProjectEntity,
  ProjectStatus,
  UpdateProjectRequest,
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

const STATUSES: { value: ProjectStatus; label: string }[] = [
  { value: "active", label: "Active" },
  { value: "paused", label: "Paused" },
  { value: "completed", label: "Completed" },
  { value: "archived", label: "Archived" },
];

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface ProjectDetailPanelProps {
  project: ProjectEntity;
  onClose: () => void;
  onUpdate: (code: string, data: UpdateProjectRequest) => void;
}

export default function ProjectDetailPanel({
  project,
  onClose,
  onUpdate,
}: ProjectDetailPanelProps) {
  // Editable fields
  const [category, setCategory] = useState<ProjectCategory>(project.category);
  const [status, setStatus] = useState<ProjectStatus>(project.status);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [extracting, setExtracting] = useState(false);

  const isDirty = category !== project.category || status !== project.status;

  // Close on Escape key
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    },
    [onClose],
  );

  useEffect(() => {
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  // Reset editable fields when project changes
  useEffect(() => {
    setCategory(project.category);
    setStatus(project.status);
    setSaveError(null);
  }, [project.category, project.status]);

  // Save handler
  const handleSave = async () => {
    if (!isDirty) return;
    setSaving(true);
    setSaveError(null);
    try {
      const data: UpdateProjectRequest = {};
      if (category !== project.category) data.category = category;
      if (status !== project.status) data.status = status;
      onUpdate(project.code, data);
    } catch (err) {
      setSaveError(
        err instanceof Error ? err.message : "Failed to save changes",
      );
    } finally {
      setSaving(false);
    }
  };

  // Re-extract handler
  const handleExtract = async () => {
    setExtracting(true);
    try {
      await apiFetch("/api/projects/extract", { method: "POST" });
    } catch {
      // Non-critical — user can retry
    } finally {
      setExtracting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/40 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Panel */}
      <div className="relative z-10 w-full md:w-[420px] h-full bg-ds-bg-100 border-l border-ds-gray-400 overflow-y-auto">
        {/* Close button */}
        <button
          type="button"
          onClick={onClose}
          className="absolute top-4 right-4 flex items-center justify-center size-8 rounded-lg text-ds-gray-900 hover:text-ds-gray-1000 hover:bg-ds-gray-100/40 transition-colors"
          aria-label="Close panel"
        >
          <X size={16} />
        </button>

        <div className="p-4 flex flex-col gap-4">
          {/* Header */}
          <div className="flex flex-col gap-2 pr-8">
            <h2 className="text-heading-16 text-ds-gray-1000">
              {project.name}
            </h2>
            <span className="text-copy-13 text-ds-gray-900 font-mono">
              {project.code}
            </span>
          </div>

          {/* Editable fields */}
          <div className="flex flex-col gap-3">
            <div className="flex flex-col gap-1">
              <label className="text-label-12 text-ds-gray-900">
                Category
              </label>
              <select
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

            <div className="flex flex-col gap-1">
              <label className="text-label-12 text-ds-gray-900">
                Status
              </label>
              <select
                value={status}
                onChange={(e) => setStatus(e.target.value as ProjectStatus)}
                className="px-3 py-2 rounded-lg bg-ds-gray-100 border border-ds-gray-400 text-copy-13 text-ds-gray-1000 focus:outline-none focus:border-ds-gray-1000/60 transition-colors"
              >
                {STATUSES.map((s) => (
                  <option key={s.value} value={s.value}>
                    {s.label}
                  </option>
                ))}
              </select>
            </div>

            {isDirty && (
              <button
                type="button"
                onClick={() => void handleSave()}
                disabled={saving}
                className="flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-button-14 text-ds-gray-1000 bg-ds-gray-alpha-200 hover:bg-ds-gray-700/30 transition-colors disabled:opacity-50"
              >
                {saving ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <Save size={14} />
                )}
                Save Changes
              </button>
            )}

            {saveError && (
              <p className="text-copy-13 text-red-700">{saveError}</p>
            )}
          </div>

          {/* Knowledge doc */}
          <div className="flex flex-col gap-2">
            <h3 className="text-label-12 text-ds-gray-900">
              Knowledge
            </h3>
            {project.content ? (
              <div className="rounded-lg bg-ds-gray-100 border border-ds-gray-400 p-4 overflow-x-auto">
                <div className="text-copy-14 text-ds-gray-1000 prose prose-sm max-w-none whitespace-pre-wrap break-words">
                  {project.content}
                </div>
              </div>
            ) : (
              <p className="text-copy-13 text-ds-gray-700">
                No knowledge document. Click Re-extract below to generate.
              </p>
            )}
          </div>

          {/* Activity stats */}
          <div className="flex flex-col gap-2">
            <h3 className="text-label-12 text-ds-gray-900">
              Activity
            </h3>
            <div className="grid grid-cols-3 gap-3">
              <div className="bg-ds-gray-100 rounded-lg p-3">
                <p className="text-copy-13 text-ds-gray-900 flex items-center gap-1">
                  <CheckSquare size={11} />
                  Obligations
                </p>
                <p className="text-copy-14 font-medium text-ds-gray-1000">
                  {project.obligation_count}
                </p>
              </div>
              <div className="bg-ds-gray-100 rounded-lg p-3">
                <p className="text-copy-13 text-ds-gray-900 flex items-center gap-1">
                  <Layers size={11} />
                  Sessions
                </p>
                <p className="text-copy-14 font-medium text-ds-gray-1000">
                  {project.session_count}
                </p>
              </div>
              <div className="bg-ds-gray-100 rounded-lg p-3">
                <p className="text-copy-13 text-ds-gray-900 flex items-center gap-1">
                  <MessageSquare size={11} />
                  Active
                </p>
                <p className="text-copy-14 font-medium text-ds-gray-1000">
                  {project.active_obligation_count}
                </p>
              </div>
            </div>
          </div>

          {/* Action buttons */}
          <div className="flex flex-col gap-2 pt-2 border-t border-ds-gray-400">
            <button
              type="button"
              onClick={() => void handleExtract()}
              disabled={extracting}
              className="flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-button-14 text-ds-gray-900 border border-ds-gray-400 hover:text-ds-gray-1000 hover:border-ds-gray-500 transition-colors disabled:opacity-50"
            >
              {extracting ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                <RefreshCw size={14} />
              )}
              Re-extract Knowledge
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
