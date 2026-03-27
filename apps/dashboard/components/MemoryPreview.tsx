"use client";

import { useMemo, useState } from "react";
import { Save, X, Edit2, FileText, AlertCircle } from "lucide-react";

export interface MemoryFile {
  name: string;
  path: string;
  content: string;
  size_bytes?: number;
  updated_at?: string;
  topics?: string[];
}

// ---------------------------------------------------------------------------
// Lightweight markdown renderer (headers, bold, italic, lists, code)
// ---------------------------------------------------------------------------

function renderMarkdown(text: string): React.ReactNode[] {
  const lines = text.split("\n");
  const result: React.ReactNode[] = [];
  let listItems: React.ReactNode[] = [];
  let listType: "ul" | "ol" | null = null;

  const flushList = () => {
    if (listItems.length > 0 && listType) {
      const Tag = listType;
      result.push(
        <Tag
          key={`list-${result.length}`}
          className={`${listType === "ul" ? "list-disc" : "list-decimal"} pl-5 my-1 space-y-0.5 text-xs text-ds-gray-900 leading-relaxed`}
        >
          {listItems}
        </Tag>,
      );
      listItems = [];
      listType = null;
    }
  };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!;

    // Headers
    const headerMatch = line.match(/^(#{1,3})\s+(.+)/);
    if (headerMatch) {
      flushList();
      const level = headerMatch[1]!.length;
      const text = formatInline(headerMatch[2]!);
      if (level === 1) {
        result.push(
          <h1
            key={i}
            className="text-copy-14 font-semibold text-ds-gray-1000 mt-3 mb-1"
          >
            {text}
          </h1>,
        );
      } else if (level === 2) {
        result.push(
          <h2
            key={i}
            className="text-copy-13 font-semibold text-ds-gray-1000 mt-2.5 mb-1"
          >
            {text}
          </h2>,
        );
      } else {
        result.push(
          <h3
            key={i}
            className="text-copy-13 font-medium text-ds-gray-1000 mt-2 mb-0.5"
          >
            {text}
          </h3>,
        );
      }
      continue;
    }

    // Unordered list items
    const ulMatch = line.match(/^[\s]*[-*]\s+(.+)/);
    if (ulMatch) {
      if (listType !== "ul") {
        flushList();
        listType = "ul";
      }
      listItems.push(<li key={i}>{formatInline(ulMatch[1]!)}</li>);
      continue;
    }

    // Ordered list items
    const olMatch = line.match(/^[\s]*\d+\.\s+(.+)/);
    if (olMatch) {
      if (listType !== "ol") {
        flushList();
        listType = "ol";
      }
      listItems.push(<li key={i}>{formatInline(olMatch[1]!)}</li>);
      continue;
    }

    flushList();

    // Empty line
    if (line.trim() === "") {
      result.push(<div key={i} className="h-2" />);
      continue;
    }

    // Regular paragraph
    result.push(
      <p key={i} className="text-xs text-ds-gray-900 leading-relaxed">
        {formatInline(line)}
      </p>,
    );
  }

  flushList();
  return result;
}

/** Format inline markdown: **bold**, *italic*, `code` */
function formatInline(text: string): React.ReactNode {
  // Split on inline patterns and rebuild with React nodes
  const parts: React.ReactNode[] = [];
  let remaining = text;
  let keyIdx = 0;

  const patterns: Array<{
    regex: RegExp;
    render: (match: string, key: number) => React.ReactNode;
  }> = [
    {
      regex: /`([^`]+)`/,
      render: (m, k) => (
        <code
          key={k}
          className="px-1 py-0.5 rounded bg-ds-gray-alpha-200 text-ds-gray-1000 font-mono text-[11px]"
        >
          {m}
        </code>
      ),
    },
    {
      regex: /\*\*([^*]+)\*\*/,
      render: (m, k) => (
        <strong key={k} className="font-semibold text-ds-gray-1000">
          {m}
        </strong>
      ),
    },
    {
      regex: /\*([^*]+)\*/,
      render: (m, k) => (
        <em key={k} className="italic">
          {m}
        </em>
      ),
    },
  ];

  while (remaining.length > 0) {
    let earliestMatch: {
      index: number;
      fullMatch: string;
      captured: string;
      render: (match: string, key: number) => React.ReactNode;
    } | null = null;

    for (const p of patterns) {
      const match = p.regex.exec(remaining);
      if (match && (earliestMatch === null || match.index < earliestMatch.index)) {
        earliestMatch = {
          index: match.index,
          fullMatch: match[0]!,
          captured: match[1]!,
          render: p.render,
        };
      }
    }

    if (!earliestMatch) {
      parts.push(remaining);
      break;
    }

    if (earliestMatch.index > 0) {
      parts.push(remaining.slice(0, earliestMatch.index));
    }
    parts.push(earliestMatch.render(earliestMatch.captured, keyIdx++));
    remaining = remaining.slice(earliestMatch.index + earliestMatch.fullMatch.length);
  }

  return parts.length === 1 ? parts[0] : <>{parts}</>;
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

  const rendered = useMemo(
    () => (file?.content ? renderMarkdown(file.content) : null),
    [file?.content],
  );

  if (!file) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-3 text-ds-gray-900">
        <FileText size={20} />
        <p className="text-copy-13">Select a file to preview</p>
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
            <p className="text-copy-14 font-medium text-ds-gray-1000 truncate">
              {file.name}
            </p>
            <p className="text-copy-13 text-ds-gray-900 font-mono truncate">
              {file.path}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {!editing && onSave && (
            <button
              type="button"
              onClick={handleEdit}
              className="flex items-center gap-1.5 px-2.5 py-1 rounded text-copy-13 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 hover:border-ds-gray-500 transition-colors"
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
                className="flex items-center gap-1.5 px-2.5 py-1 rounded text-copy-13 text-ds-gray-900 hover:text-ds-gray-1000 border border-ds-gray-400 transition-colors"
              >
                <X size={12} />
                Cancel
              </button>
              <button
                type="button"
                onClick={() => void handleSave()}
                disabled={saving}
                className="flex items-center gap-1.5 px-2.5 py-1 rounded text-button-14 font-medium bg-ds-gray-700 text-white hover:bg-ds-gray-700/80 transition-colors disabled:opacity-50"
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
            className="w-full h-full p-4 bg-transparent text-xs font-mono text-ds-gray-1000 resize-none focus:outline-hidden leading-relaxed"
            spellCheck={false}
          />
        ) : file.content ? (
          <div className="p-4 space-y-0">{rendered}</div>
        ) : (
          <div className="p-4">
            <em className="text-xs text-ds-gray-900">(empty)</em>
          </div>
        )}
      </div>
    </div>
  );
}
