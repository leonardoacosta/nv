import { db, sessions } from "@nova/db";
import { eq, and, desc } from "drizzle-orm";

export async function startSession(input: {
  name: string;
  metadata?: Record<string, unknown>;
}): Promise<string> {
  const command = input.metadata ? JSON.stringify(input.metadata) : "";

  const [row] = await db
    .insert(sessions)
    .values({
      project: input.name,
      command,
      status: "running",
    })
    .returning({ id: sessions.id });

  if (!row) {
    return "Failed to start session";
  }

  return `Session '${input.name}' started — id: ${row.id}`;
}

export async function stopSession(input: {
  name?: string;
}): Promise<string> {
  const now = new Date();

  // Find the most recent running session
  const conditions = [eq(sessions.status, "running")];
  if (input.name) {
    conditions.push(eq(sessions.project, input.name));
  }

  const [session] = await db
    .select()
    .from(sessions)
    .where(and(...conditions))
    .orderBy(desc(sessions.startedAt))
    .limit(1);

  if (!session) {
    return "No running session found";
  }

  await db
    .update(sessions)
    .set({ status: "stopped", stoppedAt: now })
    .where(eq(sessions.id, session.id));

  const durationMs = now.getTime() - session.startedAt.getTime();
  const durationMinutes = Math.round(durationMs / 60_000);

  return `Session '${session.project}' stopped — duration: ${durationMinutes}m`;
}
