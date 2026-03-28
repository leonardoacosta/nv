/**
 * Project materialization: merge projects from the daemon's project registry
 * (GET /api/projects) and from projects-* memory topics. Upsert into the
 * projects table.
 *
 * Returns { created, updated, unchanged }.
 */

import { eq, like } from "drizzle-orm";

import { db } from "@nova/db";
import { memory, projects } from "@nova/db";

export interface MaterializeResult {
  created: number;
  updated: number;
  unchanged: number;
}

interface DaemonProject {
  code: string;
  path?: string;
}

const DAEMON_TIMEOUT_MS = 3_000;

/**
 * Fetch the project registry from the Rust daemon.
 * Falls back to NV_PROJECTS env var if the daemon is unreachable.
 */
async function fetchDaemonProjects(): Promise<DaemonProject[]> {
  const daemonUrl = process.env.DAEMON_URL ?? "http://localhost:8400";
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), DAEMON_TIMEOUT_MS);

  try {
    const response = await fetch(`${daemonUrl}/api/projects`, {
      signal: controller.signal,
    });
    if (!response.ok) {
      throw new Error(`Daemon returned ${response.status}`);
    }
    const data = (await response.json()) as DaemonProject[];
    return Array.isArray(data) ? data : [];
  } catch {
    // Fall back to NV_PROJECTS env var
    const envProjects = process.env.NV_PROJECTS;
    if (envProjects) {
      try {
        return JSON.parse(envProjects) as DaemonProject[];
      } catch {
        return [];
      }
    }
    return [];
  } finally {
    clearTimeout(timeoutId);
  }
}

export async function materializeProjects(): Promise<MaterializeResult> {
  // Source 1: daemon project registry
  const daemonProjects = await fetchDaemonProjects();
  const daemonByCode = new Map<string, DaemonProject>();
  for (const p of daemonProjects) {
    if (p.code) daemonByCode.set(p.code, p);
  }

  // Source 2: projects-* memory topics
  const memoryTopics = await db
    .select({ topic: memory.topic, content: memory.content })
    .from(memory)
    .where(like(memory.topic, "projects-%"));

  const memoryByCode = new Map<string, string>();
  for (const topic of memoryTopics) {
    const code = topic.topic.replace(/^projects-/, "");
    if (code) memoryByCode.set(code, topic.content);
  }

  // Merge: deduplicate by project code
  const allCodes = new Set<string>([
    ...daemonByCode.keys(),
    ...memoryByCode.keys(),
  ]);

  if (allCodes.size === 0) {
    return { created: 0, updated: 0, unchanged: 0 };
  }

  // Load existing projects for matching
  const existingProjects = await db.select().from(projects);
  const existingByCode = new Map<
    string,
    (typeof existingProjects)[number]
  >();
  for (const p of existingProjects) {
    existingByCode.set(p.code, p);
  }

  let created = 0;
  let updated = 0;
  let unchanged = 0;

  for (const code of allCodes) {
    const daemonEntry = daemonByCode.get(code);
    const memoryContent = memoryByCode.get(code);
    const description = memoryContent
      ? memoryContent.slice(0, 500)
      : null;

    const existing = existingByCode.get(code);

    if (existing) {
      // Update: set path from daemon if null, description from memory if null
      const updates: Record<string, unknown> = {};

      if (!existing.path && daemonEntry?.path) {
        updates.path = daemonEntry.path;
      }
      if (!existing.description && description) {
        updates.description = description;
      }

      if (Object.keys(updates).length === 0) {
        unchanged++;
      } else {
        updates.updatedAt = new Date();
        await db
          .update(projects)
          .set(updates)
          .where(eq(projects.id, existing.id));
        updated++;
      }
    } else {
      // Insert new project
      await db
        .insert(projects)
        .values({
          code,
          name: code,
          category: "work",
          status: "active",
          path: daemonEntry?.path ?? null,
          description,
        })
        .onConflictDoNothing();
      created++;
    }
  }

  return { created, updated, unchanged };
}
