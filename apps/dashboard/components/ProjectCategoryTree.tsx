"use client";

import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { ProjectCategory, ProjectEntity } from "@/types/api";
import ProjectCard from "@/components/ProjectCard";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const CATEGORY_ORDER: ProjectCategory[] = [
  "work",
  "personal",
  "open_source",
  "archived",
];

const CATEGORY_LABEL: Record<ProjectCategory, string> = {
  work: "Work",
  personal: "Personal",
  open_source: "Open Source",
  archived: "Archived",
};

// ---------------------------------------------------------------------------
// CategoryNode
// ---------------------------------------------------------------------------

function CategoryNode({
  category,
  projects,
  onSelectProject,
}: {
  category: ProjectCategory;
  projects: ProjectEntity[];
  onSelectProject: (code: string) => void;
}) {
  const [open, setOpen] = useState(true);

  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-2 py-1 hover:text-ds-gray-1000 transition-colors"
      >
        {open ? (
          <ChevronDown size={14} className="text-ds-gray-700 shrink-0" />
        ) : (
          <ChevronRight size={14} className="text-ds-gray-700 shrink-0" />
        )}
        <span className="text-label-14 font-medium text-ds-gray-1000">
          {CATEGORY_LABEL[category]}
        </span>
        <span
          className="inline-flex items-center justify-center px-1.5 py-0.5 min-w-[1.25rem] rounded text-xs font-mono font-medium text-ds-gray-900"
          style={{ background: "var(--ds-gray-alpha-200)" }}
        >
          {projects.length}
        </span>
      </button>

      {open && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3 pl-5">
          {projects.map((project) => (
            <ProjectCard
              key={project.id}
              project={project}
              onClick={() => onSelectProject(project.code)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

interface ProjectCategoryTreeProps {
  projects: ProjectEntity[];
  onSelectProject: (code: string) => void;
}

export default function ProjectCategoryTree({
  projects,
  onSelectProject,
}: ProjectCategoryTreeProps) {
  // Group by category
  const grouped = new Map<ProjectCategory, ProjectEntity[]>();
  for (const p of projects) {
    const cat = p.category;
    if (!grouped.has(cat)) grouped.set(cat, []);
    grouped.get(cat)!.push(p);
  }

  return (
    <div className="flex flex-col gap-4">
      {CATEGORY_ORDER.filter((cat) => grouped.has(cat)).map((cat) => (
        <CategoryNode
          key={cat}
          category={cat}
          projects={grouped.get(cat)!}
          onSelectProject={onSelectProject}
        />
      ))}
    </div>
  );
}
