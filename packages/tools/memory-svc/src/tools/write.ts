import type { Context } from "hono";
import { eq, sql } from "drizzle-orm";
import { db, memory } from "@nova/db";
import { generateEmbedding } from "../embedding.js";
import type { MemorySvcConfig } from "../config.js";
import type { Logger } from "../logger.js";

export async function handleWrite(
  c: Context,
  config: MemorySvcConfig,
  logger: Logger,
) {
  const body = await c.req.json<{ topic?: string; content?: string }>();
  const { topic, content } = body;

  if (!topic || typeof topic !== "string") {
    return c.json({ error: "topic is required" }, 400);
  }
  if (!content || typeof content !== "string") {
    return c.json({ error: "content is required" }, 400);
  }

  // Check if topic already exists to determine create vs update
  const existing = await db.query.memory.findFirst({
    where: eq(memory.topic, topic),
    columns: { id: true },
  });

  // Generate embedding (best-effort)
  const embedding = await generateEmbedding(
    `${topic}\n\n${content}`,
    config.openaiApiKey,
    logger,
  );

  // Upsert into Postgres
  await db
    .insert(memory)
    .values({
      topic,
      content,
      embedding: embedding ?? undefined,
      updatedAt: new Date(),
    })
    .onConflictDoUpdate({
      target: memory.topic,
      set: {
        content,
        embedding: embedding ?? sql`memory.embedding`,
        updatedAt: new Date(),
      },
    });

  const action = existing ? "updated" : "created";

  return c.json({ topic, action });
}
