import { db, obligations } from "@nova/db";
import { desc, or, eq } from "drizzle-orm";

const TELEGRAM_MAX_CHARS = 4000;

/**
 * /ob — list active obligations from DB
 */
export async function buildObReply(): Promise<string> {
  const rows = await db
    .select()
    .from(obligations)
    .where(
      or(
        eq(obligations.status, "open"),
        eq(obligations.status, "in_progress"),
      ),
    )
    .orderBy(desc(obligations.createdAt))
    .limit(20);

  if (rows.length === 0) {
    return "No active obligations.";
  }

  const header = `Active Obligations (${rows.length})\n${"─".repeat(32)}\n`;
  const lines = rows.map((o) => {
    const prio = `P${o.priority}`;
    const owner = o.owner;
    const deadline = o.deadline
      ? ` due ${o.deadline.toISOString().slice(0, 10)}`
      : "";
    const status = o.status === "in_progress" ? " [in progress]" : "";
    const action =
      o.detectedAction.length > 60
        ? o.detectedAction.slice(0, 57) + "..."
        : o.detectedAction;
    return `  [${prio}] ${action}\n    owner: ${owner}${deadline}${status}`;
  });

  return truncate(header + lines.join("\n\n"));
}

function truncate(text: string): string {
  if (text.length <= TELEGRAM_MAX_CHARS) return text;
  return text.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
