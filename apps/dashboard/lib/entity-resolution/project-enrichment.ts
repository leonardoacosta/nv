/**
 * Project enrichment (LEGACY).
 *
 * @deprecated Use the DB-backed extraction pipeline at `/api/projects/extract`
 * instead. Projects are now stored in the `projects` table and enriched
 * via the extract endpoint. This file is kept for backward compatibility
 * but should not be used in new code.
 *
 * Takes a base list of ApiProject records (from NV_PROJECTS env var) and
 * enriches each with live DB data:
 *  - Obligation counts (total + active)
 *  - Session counts + most recent startedAt
 *  - Memory context from `projects-*` topics
 */

import { count, eq, inArray, like, max } from "drizzle-orm";
import { obligations, sessions, memory } from "@nova/db";
import type { db as DrizzleClient } from "@nova/db";

export interface ApiProject {
  code: string;
  path: string;
}

export interface EnrichedProject {
  code: string;
  path: string;
  description: string | null;
  memoryContext: string | null;
  obligationCount: number;
  activeObligationCount: number;
  sessionCount: number;
  lastActivity: string | null;
}

const ACTIVE_STATUSES = ["open", "in_progress"];

/**
 * Enrich a list of projects with obligation counts, session counts, and
 * memory context.
 *
 * @param projects  Base project list from NV_PROJECTS
 * @param db        Drizzle client instance
 */
export async function enrichProjects(
  projects: ApiProject[],
  db: typeof DrizzleClient,
): Promise<EnrichedProject[]> {
  if (projects.length === 0) return [];

  const codes = projects.map((p) => p.code);

  // ── 1. Obligation counts per project ────────────────────────────────────

  // Total count per project_code
  const obligationTotals = await db
    .select({
      projectCode: obligations.projectCode,
      total: count(obligations.id),
    })
    .from(obligations)
    .where(inArray(obligations.projectCode, codes))
    .groupBy(obligations.projectCode);

  // Active count per project_code
  const obligationActive = await db
    .select({
      projectCode: obligations.projectCode,
      activeTotal: count(obligations.id),
    })
    .from(obligations)
    .where(
      inArray(obligations.projectCode, codes) &&
        inArray(obligations.status, ACTIVE_STATUSES),
    )
    .groupBy(obligations.projectCode);

  // ── 2. Session counts + last startedAt per project ──────────────────────

  const sessionStats = await db
    .select({
      project: sessions.project,
      sessionCount: count(sessions.id),
      lastStarted: max(sessions.startedAt),
    })
    .from(sessions)
    .where(inArray(sessions.project, codes))
    .groupBy(sessions.project);

  // ── 3. Memory topics matching `projects-*` ───────────────────────────────

  const projectMemoryTopics = await db
    .select({
      topic: memory.topic,
      content: memory.content,
    })
    .from(memory)
    .where(like(memory.topic, "projects-%"));

  // ── 4. Build lookup maps ─────────────────────────────────────────────────

  const totalByCode = new Map<string, number>();
  for (const row of obligationTotals) {
    if (row.projectCode) totalByCode.set(row.projectCode, row.total);
  }

  const activeByCode = new Map<string, number>();
  for (const row of obligationActive) {
    if (row.projectCode) activeByCode.set(row.projectCode, row.activeTotal);
  }

  const sessionByCode = new Map<
    string,
    { sessionCount: number; lastStarted: Date | null }
  >();
  for (const row of sessionStats) {
    sessionByCode.set(row.project, {
      sessionCount: row.sessionCount,
      lastStarted: row.lastStarted ?? null,
    });
  }

  // ── 5. Match memory topics to projects ──────────────────────────────────

  function findMemoryForProject(code: string): {
    description: string | null;
    memoryContext: string | null;
  } {
    // Find any `projects-*` topic whose content mentions this project code
    const matches = projectMemoryTopics.filter((t) =>
      t.content.toLowerCase().includes(code.toLowerCase()),
    );

    if (matches.length === 0) return { description: null, memoryContext: null };

    // Prefer a topic whose name includes the code (e.g. `projects-nv`)
    const preferred =
      matches.find((t) => t.topic.toLowerCase().includes(code.toLowerCase())) ??
      matches[0];

    const preview = preferred.content.slice(0, 500);

    // Attempt a one-line description: first non-empty line after the code mention
    const lines = preferred.content.split("\n");
    const descLine = lines.find(
      (l) => l.trim() && !l.toLowerCase().startsWith("#") && l.trim().length > 10,
    );

    return {
      description: descLine?.trim() ?? null,
      memoryContext: preview,
    };
  }

  // ── 6. Assemble enriched projects ────────────────────────────────────────

  return projects.map((project) => {
    const sessionInfo = sessionByCode.get(project.code);
    const { description, memoryContext } = findMemoryForProject(project.code);

    // lastActivity = max of session.lastStarted (obligation.updatedAt not
    // aggregated here to keep query count low — sessions cover most cases)
    const lastActivity = sessionInfo?.lastStarted
      ? sessionInfo.lastStarted.toISOString()
      : null;

    return {
      code: project.code,
      path: project.path,
      description,
      memoryContext,
      obligationCount: totalByCode.get(project.code) ?? 0,
      activeObligationCount: activeByCode.get(project.code) ?? 0,
      sessionCount: sessionInfo?.sessionCount ?? 0,
      lastActivity,
    };
  });
}
