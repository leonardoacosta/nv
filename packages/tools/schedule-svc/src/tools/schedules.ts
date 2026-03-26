import { db, schedules } from "@nova/db";
import { eq } from "drizzle-orm";

import { validateCron } from "./cron.js";

export async function addSchedule(input: {
  name: string;
  cron: string;
  action: string;
}): Promise<string> {
  if (!validateCron(input.cron)) {
    return `Invalid cron expression: '${input.cron}'. Expected 5-field standard cron (minute hour day-of-month month day-of-week).`;
  }

  try {
    const [row] = await db
      .insert(schedules)
      .values({
        name: input.name,
        cronExpr: input.cron,
        action: input.action,
        channel: "schedule-svc",
        enabled: true,
      })
      .returning({ id: schedules.id });

    if (!row) {
      return "Failed to create schedule";
    }

    return `Schedule '${input.name}' created — id: ${row.id}`;
  } catch (err: unknown) {
    // Unique constraint violation on name
    if (
      err instanceof Error &&
      err.message.includes("unique")
    ) {
      return `Schedule '${input.name}' already exists`;
    }
    throw err;
  }
}

export async function modifySchedule(input: {
  id: string;
  updates: {
    name?: string;
    cron?: string;
    action?: string;
    enabled?: boolean;
  };
}): Promise<string> {
  if (input.updates.cron !== undefined && !validateCron(input.updates.cron)) {
    return `Invalid cron expression: '${input.updates.cron}'. Expected 5-field standard cron (minute hour day-of-month month day-of-week).`;
  }

  const setFields: Record<string, unknown> = {};
  if (input.updates.name !== undefined) setFields["name"] = input.updates.name;
  if (input.updates.cron !== undefined)
    setFields["cronExpr"] = input.updates.cron;
  if (input.updates.action !== undefined)
    setFields["action"] = input.updates.action;
  if (input.updates.enabled !== undefined)
    setFields["enabled"] = input.updates.enabled;

  if (Object.keys(setFields).length === 0) {
    return `Schedule ${input.id} updated`;
  }

  const [row] = await db
    .update(schedules)
    .set(setFields)
    .where(eq(schedules.id, input.id))
    .returning({ id: schedules.id });

  if (!row) {
    return `Schedule ${input.id} not found`;
  }

  return `Schedule ${input.id} updated`;
}

export async function removeSchedule(input: { id: string }): Promise<string> {
  const [row] = await db
    .delete(schedules)
    .where(eq(schedules.id, input.id))
    .returning({ id: schedules.id });

  if (!row) {
    return `Schedule ${input.id} not found`;
  }

  return `Schedule ${input.id} removed`;
}

export async function listSchedules(input: {
  active?: boolean;
}): Promise<string> {
  const active = input.active ?? true;

  const rows = active
    ? await db
        .select()
        .from(schedules)
        .where(eq(schedules.enabled, true))
        .orderBy(schedules.name)
    : await db.select().from(schedules).orderBy(schedules.name);

  return JSON.stringify(rows);
}
