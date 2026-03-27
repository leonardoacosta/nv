"use client";

import { useCallback, useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@nova/ui";
import { Button } from "@nova/ui";
import { Input } from "@nova/ui";
import { Label } from "@nova/ui";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@nova/ui";
import type {
  CreateProjectRequest,
  ProjectCategory,
  ProjectEntity,
} from "@/types/api";
import { trpcClient } from "@/lib/trpc/client";

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

      const project = (await trpcClient.project.create.mutate(body)) as ProjectEntity;
      onCreated(project);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create project");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(isOpen) => { if (!isOpen) onClose(); }}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Create Project</DialogTitle>
          <DialogDescription className="sr-only">
            Create a new project with a code, name, and category.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={(e) => void handleSubmit(e)} className="p-5 pt-0 flex flex-col gap-4">
          {/* Code */}
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="project-code" className="text-label-12 text-ds-gray-900">
              Code
            </Label>
            <Input
              id="project-code"
              type="text"
              value={code}
              onChange={(e) => setCode(e.target.value.toLowerCase())}
              placeholder="my-project"
              className="font-mono"
              autoFocus
            />
            {code.length > 0 && !codeValid && (
              <p className="text-copy-13 text-red-700">
                Must be kebab-case (lowercase letters, numbers, hyphens)
              </p>
            )}
          </div>

          {/* Name */}
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="project-name" className="text-label-12 text-ds-gray-900">
              Name
            </Label>
            <Input
              id="project-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My Project"
            />
          </div>

          {/* Category */}
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="project-category" className="text-label-12 text-ds-gray-900">
              Category
            </Label>
            <Select
              value={category}
              onValueChange={(v) => setCategory(v as ProjectCategory)}
            >
              <SelectTrigger id="project-category">
                <SelectValue placeholder="Select category" />
              </SelectTrigger>
              <SelectContent>
                {CATEGORIES.map((c) => (
                  <SelectItem key={c.value} value={c.value}>
                    {c.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {/* Path (optional) */}
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="project-path" className="text-label-12 text-ds-gray-900">
              Path (optional)
            </Label>
            <Input
              id="project-path"
              type="text"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="~/dev/my-project"
              className="font-mono"
            />
          </div>

          {/* Error */}
          {error && (
            <p className="text-copy-13 text-red-700">{error}</p>
          )}

          {/* Submit */}
          <Button type="submit" disabled={!canSubmit}>
            {submitting && <Loader2 size={14} className="animate-spin" />}
            Create Project
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  );
}
