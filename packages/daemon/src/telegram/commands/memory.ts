import { db, memory } from "@nova/db";
import { desc } from "drizzle-orm";
import { eq } from "drizzle-orm";

const TELEGRAM_MAX_CHARS = 4000;

/**
 * /memory — list all topics
 * /memory [topic] — read a specific topic
 */
export async function buildMemoryReply(topicArg?: string): Promise<string> {
  if (!topicArg) {
    const rows = await db
      .select({ topic: memory.topic, updatedAt: memory.updatedAt })
      .from(memory)
      .orderBy(desc(memory.updatedAt));

    if (rows.length === 0) {
      return "No memory topics found.";
    }

    const header = `Memory Topics (${rows.length})\n${"─".repeat(32)}\n`;
    const list = rows
      .map((r) => {
        const date = r.updatedAt.toISOString().slice(0, 10);
        return `  ${r.topic} (${date})`;
      })
      .join("\n");

    return truncate(header + list);
  }

  const row = await db
    .select()
    .from(memory)
    .where(eq(memory.topic, topicArg))
    .limit(1);

  const record = row[0];
  if (!record) {
    return `No memory found for topic: ${topicArg}`;
  }

  const header = `Memory: ${record.topic}\nUpdated: ${record.updatedAt.toISOString().slice(0, 10)}\n${"─".repeat(32)}\n`;
  return truncate(header + record.content);
}

function truncate(text: string): string {
  if (text.length <= TELEGRAM_MAX_CHARS) return text;
  return text.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
