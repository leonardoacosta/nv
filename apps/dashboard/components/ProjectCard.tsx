"use client";

import { CheckSquare, Clock, FolderOpen, Layers } from "lucide-react";
import type { ProjectEntity } from "@/types/api";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function relativeTime(iso: string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diff = now - then;
  if (diff < 0) return "just now";
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  const weeks = Math.floor(days / 7);
  if (weeks < 5) return `${weeks}w ago`;
  const months = Math.floor(days / 30);
  if (months < 12) return `${months}mo ago`;
  return ">1y ago";
}

const STATUS_DOT: Record<string, string> = {
  active: "bg-green-700",
  paused: "bg-amber-700",
  completed: "bg-blue-700",
  archived: "bg-ds-gray-600",
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface ProjectCardProps {
  project: ProjectEntity;
  onClick: () => void;
}

export default function ProjectCard({ project, onClick }: ProjectCardProps) {
  const dot = STATUS_DOT[project.status] ?? "bg-ds-gray-600";

  return (
    <button
      type="button"
      onClick={onClick}
      className="w-full text-left surface-card p-4 hover:border-ds-gray-1000/40 transition-colors flex flex-col gap-2"
    >
      {/* Name + status */}
      <div className="flex items-center gap-2">
        <FolderOpen size={14} className="text-ds-gray-900 shrink-0" />
        <span className="text-copy-14 font-medium text-ds-gray-1000 truncate">
          {project.name}
        </span>
        <span className={`inline-block size-2 rounded-full shrink-0 ${dot}`} />
      </div>

      {/* Description */}
      {project.description && (
        <p className="text-copy-13 text-ds-gray-900 truncate">
          {project.description}
        </p>
      )}

      {/* Badges + last activity */}
      <div className="flex items-center gap-3 flex-wrap text-copy-13 text-ds-gray-900">
        {project.active_obligation_count > 0 && (
          <span className="flex items-center gap-1">
            <CheckSquare size={11} />
            {project.active_obligation_count} obligation{project.active_obligation_count !== 1 ? "s" : ""}
          </span>
        )}
        {project.session_count > 0 && (
          <span className="flex items-center gap-1">
            <Layers size={11} />
            {project.session_count} session{project.session_count !== 1 ? "s" : ""}
          </span>
        )}
        {project.last_activity && (
          <span
            className="flex items-center gap-1 font-mono"
            suppressHydrationWarning
          >
            <Clock size={11} />
            {relativeTime(project.last_activity)}
          </span>
        )}
      </div>
    </button>
  );
}
