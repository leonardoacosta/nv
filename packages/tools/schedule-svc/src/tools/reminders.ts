import { db, reminders } from "@nova/db";
import { eq, and, isNull } from "drizzle-orm";

export async function setReminder(input: {
  description: string;
  due_at: string;
}): Promise<string> {
  const dueAt = new Date(input.due_at);

  const [row] = await db
    .insert(reminders)
    .values({
      message: input.description,
      dueAt,
      channel: "schedule-svc",
      cancelled: false,
    })
    .returning({ id: reminders.id });

  if (!row) {
    return "Failed to create reminder";
  }

  return `Reminder set for ${input.due_at} — id: ${row.id}`;
}

export async function cancelReminder(input: { id: string }): Promise<string> {
  const [row] = await db
    .update(reminders)
    .set({ cancelled: true })
    .where(eq(reminders.id, input.id))
    .returning({ id: reminders.id });

  if (!row) {
    return `Reminder ${input.id} not found`;
  }

  return `Reminder ${input.id} cancelled`;
}

export async function listReminders(input: {
  status?: "active" | "all";
}): Promise<string> {
  const status = input.status ?? "active";

  const rows =
    status === "active"
      ? await db
          .select()
          .from(reminders)
          .where(
            and(
              eq(reminders.cancelled, false),
              isNull(reminders.deliveredAt),
            ),
          )
          .orderBy(reminders.dueAt)
      : await db.select().from(reminders).orderBy(reminders.dueAt);

  return JSON.stringify(rows);
}
