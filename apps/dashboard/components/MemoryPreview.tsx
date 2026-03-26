"use client";

import { useState } from "react";
import { Save, X, Edit2, FileText, AlertCircle } from "lucide-react";

export interface MemoryFile {
  name: string;
  path: string;
  content: string;
  size_bytes?: number;
  updated_at?: string;
  topics?: string[];
}

interface MemoryPreviewProps {
  file: MemoryFile | null;
  onSave?: (path: string, content: string) => Promise<void>;
  onClose?: () => void;
}

export default function MemoryPreview({
  file,
  onSave,
  onClose,
}: MemoryPreviewProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  if (!file) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-3 text-ds-gray-900">
        <FileText size={36} />
        <p className="text-sm">Select a file to preview</p>
      </div>
    );
  }

  const handleEdit = () => {
    setDraft(file.content);
    setSaveError(null);
    setEditing(true);
  };

  const handleCancel = () => {
    setEditing(false);
    setSaveError(null);
  };

  const handleSave = async () => {
    if (!onSave) return;
    setSaving(true);
    setSaveError(null);
    try {
      await onSave(file.path, draft);
      setEditing(false);
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : "Failed to save");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex flex-col h-full rounded-xl border border-ds-gray-400 bg-ds-gray-100 overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-ds-gray-400 shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          <FileText size={14} className="text-ds-gray-1000 shrink-0" />
          <div className="min-w-0">
            <p className="text-sm font-medium text-ds-gray-1000 truncate">
              {file.name}
            </p>
            <p className="text-xs text-ds-gray-900 font-mono truncate">
              {file.path}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {!editing && onSave && (
            <button
              type="button"
              onClick={handleEdit}
              className="flex items-center gap-1.5 px-2.5 py-1 rounded text-xs text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors"
            >
              <Edit2 size={12} />
              Edit
            </button>
          )}
          {editing && (
            <>
              <button
                type="button"
                onClick={handleCancel}
                className="flex items-center gap-1.5 px-2.5 py-1 rounded text-xs text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 transition-colors"
              >
                <X size={12} />
                Cancel
              </button>
              <button
                type="button"
                onClick={() => void handleSave()}
                disabled={saving}
                className="flex items-center gap-1.5 px-2.5 py-1 rounded text-xs font-medium bg-ds-gray-700 text-white hover:bg-ds-gray-700/80 transition-colors disabled:opacity-50"
              >
                <Save size={12} />
                {saving ? "Saving..." : "Save"}
              </button>
            </>
          )}
          {onClose && (
            <button
              type="button"
              onClick={onClose}
              className="flex items-center justify-center w-6 h-6 rounded text-ds-gray-900 hover:text-ds-gray-1000 transition-colors"
            >
              <X size={13} />
            </button>
          )}
        </div>
      </div>

      {/* Topics */}
      {file.topics && file.topics.length > 0 && (
        <div className="flex items-center gap-2 px-4 py-2 border-b border-ds-gray-400 bg-ds-bg-100/30 flex-wrap">
          {file.topics.map((topic) => (
            <span
              key={topic}
              className="text-xs px-2 py-0.5 rounded bg-ds-gray-alpha-200 text-ds-gray-1000 font-mono"
            >
              {topic}
            </span>
          ))}
        </div>
      )}

      {/* Meta */}
      <div className="flex items-center gap-4 px-4 py-2 border-b border-ds-gray-400 text-xs text-ds-gray-900 font-mono">
        {file.size_bytes !== undefined && (
          <span>{(file.size_bytes / 1024).toFixed(1)} KB</span>
        )}
        {file.updated_at && (
          <span suppressHydrationWarning>
            Updated {new Date(file.updated_at).toLocaleDateString()}
          </span>
        )}
      </div>

      {/* Error */}
      {saveError && (
        <div className="flex items-center gap-2 mx-4 mt-2 p-2 rounded bg-red-700/10 border border-red-700/30 text-red-700 text-xs">
          <AlertCircle size={12} />
          {saveError}
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-auto">
        {editing ? (
          <textarea
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            className="w-full h-full p-4 bg-transparent text-xs font-mono text-ds-gray-1000 resize-none focus:outline-none leading-relaxed"
            spellCheck={false}
          />
        ) : (
          <pre className="p-4 text-xs font-mono text-ds-gray-900 leading-relaxed whitespace-pre-wrap break-words">
            {file.content || <em className="text-ds-gray-900">(empty)</em>}
          </pre>
        )}
      </div>
    </div>
  );
}
