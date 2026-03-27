import { db, memory } from "@nova/db";
import type { DreamOrientation, TopicStats } from "./types.js";

/**
 * Orient phase: read all memory rows from Postgres and compute per-topic stats.
 * Returns a snapshot of every topic with size, line count, and last-updated time.
 */
export async function orient(): Promise<DreamOrientation> {
  const rows = await db.select().from(memory);

  const topics: TopicStats[] = rows.map((row) => ({
    topic: row.topic,
    sizeBytes: Buffer.byteLength(row.content, "utf-8"),
    lineCount: row.content.split("\n").length,
    updatedAt: row.updatedAt,
  }));

  const totalSizeBytes = topics.reduce((sum, t) => sum + t.sizeBytes, 0);

  return {
    topics,
    totalSizeBytes,
    timestamp: new Date(),
  };
}
