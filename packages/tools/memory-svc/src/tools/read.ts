import type { Context } from "hono";
import { eq } from "drizzle-orm";
import { db, memory } from "@nova/db";
import { readMemoryFile } from "../filesystem.js";
import type { MemorySvcConfig } from "../config.js";

const MAX_MEMORY_READ_CHARS = 20_000;

function truncate(content: string): string {
  if (content.length <= MAX_MEMORY_READ_CHARS) return content;
  return content.slice(0, MAX_MEMORY_READ_CHARS);
}

export async function handleRead(c: Context, config: MemorySvcConfig) {
  const body = await c.req.json<{ topic?: string }>();
  const topic = body.topic;

  if (!topic || typeof topic !== "string") {
    return c.json({ error: "topic is required" }, 400);
  }

  // Query Postgres first
  const row = await db.query.memory.findFirst({
    where: eq(memory.topic, topic),
  });

  if (row) {
    return c.json({
      topic: row.topic,
      content: truncate(row.content),
      updatedAt: row.updatedAt.toISOString(),
    });
  }

  // Fallback to filesystem
  const fileResult = await readMemoryFile(config.memoryDir, topic);
  if (fileResult) {
    return c.json({
      topic,
      content: truncate(fileResult.content),
      updatedAt: fileResult.updatedAt.toISOString(),
    });
  }

  return c.json({ error: "not_found" }, 404);
}
