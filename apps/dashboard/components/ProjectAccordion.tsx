"use client";

import { useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  AlertCircle,
  Zap,
  FileCode,
} from "lucide-react";

export interface ProjectError {
  id: string;
  file?: string;
  line?: number;
  message: string;
  severity: "error" | "warning" | "info";
}

export interface Project {
  id: string;
  name: string;
  path?: string;
  status: "healthy" | "errors" | "warnings" | "unknown";
  errors?: ProjectError[];
  nova_notes?: string;
  language?: string;
  last_checked?: string;
}

const STATUS_CONFIG: Record<
  Project["status"],
  { label: string; dot: string; text: string }
> = {
  healthy: {
    label: "Healthy",
    dot: "bg-green-700",
    text: "text-green-700",
  },
  errors: { label: "Errors", dot: "bg-red-700", text: "text-red-700" },
  warnings: {
    label: "Warnings",
    dot: "bg-amber-700",
    text: "text-amber-700",
  },
  unknown: {
    label: "Unknown",
    dot: "bg-ds-gray-600",
    text: "text-ds-gray-900",
  },
};

const SEVERITY_COLORS = {
  error: "text-red-700 bg-red-700/10",
  warning: "text-amber-700 bg-amber-700/10",
  info: "text-ds-gray-1000 bg-ds-gray-alpha-100",
};

interface ProjectAccordionProps {
  project: Project;
  onSolveWithNexus?: (projectId: string, errorId: string) => void;
}

export default function ProjectAccordion({
  project,
  onSolveWithNexus,
}: ProjectAccordionProps) {
  const [open, setOpen] = useState(false);
  const cfg = STATUS_CONFIG[project.status];
  const errorCount = project.errors?.length ?? 0;

  return (
    <div className="rounded-xl border border-ds-gray-400 bg-ds-gray-100 overflow-hidden">
      {/* Header row */}
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="w-full flex items-center gap-3 px-4 py-3.5 hover:bg-ds-gray-alpha-200 transition-colors text-left"
      >
        <div className="shrink-0 text-ds-gray-900">
          {open ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
        </div>

        <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-ds-bg-100 shrink-0">
          <FileCode size={16} className="text-ds-gray-1000" />
        </div>

        <div className="flex-1 min-w-0">
          <p className="text-copy-14 font-medium text-ds-gray-1000 truncate">
            {project.name}
          </p>
          {project.path && (
            <p className="text-copy-13 text-ds-gray-900 font-mono truncate">
              {project.path}
            </p>
          )}
        </div>

        <div className="flex items-center gap-3 shrink-0">
          {errorCount > 0 && (
            <span className="flex items-center gap-1 text-copy-13 font-mono text-red-700">
              <AlertCircle size={12} />
              {errorCount}
            </span>
          )}
          <div className="flex items-center gap-1.5">
            <div className={`w-2 h-2 rounded-full ${cfg.dot}`} />
            <span className={`text-label-12 font-medium ${cfg.text}`}>
              {cfg.label}
            </span>
          </div>
        </div>
      </button>

      {/* Expanded content */}
      {open && (
        <div className="border-t border-ds-gray-400">
          {/* Nova notes */}
          {project.nova_notes && (
            <div className="flex items-start gap-3 px-4 py-3 bg-ds-gray-700/5 border-b border-ds-gray-400">
              <div className="w-5 h-5 rounded bg-ds-gray-700/30 flex items-center justify-center shrink-0 mt-0.5">
                <span className="text-xs font-bold font-mono text-ds-gray-1000">
                  N
                </span>
              </div>
              <p className="text-copy-13 text-ds-gray-900 leading-relaxed">
                {project.nova_notes}
              </p>
            </div>
          )}

          {/* Errors list */}
          {errorCount === 0 ? (
            <div className="px-4 py-6 text-center text-copy-13 text-ds-gray-900">
              No issues detected
            </div>
          ) : (
            <div className="divide-y divide-ds-gray-400">
              {project.errors!.map((err) => (
                <div key={err.id} className="flex items-start gap-3 px-4 py-3">
                  <div
                    className={`mt-0.5 px-1.5 py-0.5 rounded text-label-12 font-mono font-medium ${SEVERITY_COLORS[err.severity]}`}
                  >
                    {err.severity}
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-copy-13 text-ds-gray-1000">{err.message}</p>
                    {err.file && (
                      <p className="text-copy-13 text-ds-gray-900 font-mono mt-0.5">
                        {err.file}
                        {err.line ? `:${err.line}` : ""}
                      </p>
                    )}
                  </div>
                  {onSolveWithNexus && (
                    <button
                      type="button"
                      onClick={() => onSolveWithNexus(project.id, err.id)}
                      className="flex items-center gap-1.5 px-2.5 py-1 rounded text-xs font-medium bg-ds-gray-alpha-200 text-ds-gray-1000 hover:bg-ds-gray-700/30 transition-colors shrink-0"
                    >
                      <Zap size={11} />
                      Solve with Nexus
                    </button>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
