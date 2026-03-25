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
    dot: "bg-emerald-500",
    text: "text-emerald-400",
  },
  errors: { label: "Errors", dot: "bg-[#EF4444]", text: "text-[#EF4444]" },
  warnings: {
    label: "Warnings",
    dot: "bg-[#F97316]",
    text: "text-[#F97316]",
  },
  unknown: {
    label: "Unknown",
    dot: "bg-cosmic-muted",
    text: "text-cosmic-muted",
  },
};

const SEVERITY_COLORS = {
  error: "text-[#EF4444] bg-[#EF4444]/10",
  warning: "text-[#F97316] bg-[#F97316]/10",
  info: "text-cosmic-purple bg-cosmic-purple/10",
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
    <div className="rounded-cosmic border border-cosmic-border bg-cosmic-surface overflow-hidden">
      {/* Header row */}
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="w-full flex items-center gap-3 px-4 py-3.5 hover:bg-cosmic-border/20 transition-colors text-left"
      >
        <div className="shrink-0 text-cosmic-muted">
          {open ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
        </div>

        <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-cosmic-dark shrink-0">
          <FileCode size={16} className="text-cosmic-purple" />
        </div>

        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium text-cosmic-bright truncate">
            {project.name}
          </p>
          {project.path && (
            <p className="text-xs text-cosmic-muted font-mono truncate">
              {project.path}
            </p>
          )}
        </div>

        <div className="flex items-center gap-3 shrink-0">
          {errorCount > 0 && (
            <span className="flex items-center gap-1 text-xs font-mono text-[#EF4444]">
              <AlertCircle size={12} />
              {errorCount}
            </span>
          )}
          <div className="flex items-center gap-1.5">
            <div className={`w-2 h-2 rounded-full ${cfg.dot}`} />
            <span className={`text-xs font-medium ${cfg.text}`}>
              {cfg.label}
            </span>
          </div>
        </div>
      </button>

      {/* Expanded content */}
      {open && (
        <div className="border-t border-cosmic-border">
          {/* Nova notes */}
          {project.nova_notes && (
            <div className="flex items-start gap-3 px-4 py-3 bg-cosmic-purple/5 border-b border-cosmic-border">
              <div className="w-5 h-5 rounded bg-cosmic-purple/30 flex items-center justify-center shrink-0 mt-0.5">
                <span className="text-xs font-bold font-mono text-cosmic-purple">
                  N
                </span>
              </div>
              <p className="text-xs text-cosmic-muted leading-relaxed">
                {project.nova_notes}
              </p>
            </div>
          )}

          {/* Errors list */}
          {errorCount === 0 ? (
            <div className="px-4 py-6 text-center text-sm text-cosmic-muted">
              No issues detected
            </div>
          ) : (
            <div className="divide-y divide-cosmic-border">
              {project.errors!.map((err) => (
                <div key={err.id} className="flex items-start gap-3 px-4 py-3">
                  <div
                    className={`mt-0.5 px-1.5 py-0.5 rounded text-xs font-mono font-medium ${SEVERITY_COLORS[err.severity]}`}
                  >
                    {err.severity}
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-xs text-cosmic-text">{err.message}</p>
                    {err.file && (
                      <p className="text-xs text-cosmic-muted font-mono mt-0.5">
                        {err.file}
                        {err.line ? `:${err.line}` : ""}
                      </p>
                    )}
                  </div>
                  {onSolveWithNexus && (
                    <button
                      type="button"
                      onClick={() => onSolveWithNexus(project.id, err.id)}
                      className="flex items-center gap-1.5 px-2.5 py-1 rounded text-xs font-medium bg-cosmic-purple/20 text-cosmic-purple hover:bg-cosmic-purple/30 transition-colors shrink-0"
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
