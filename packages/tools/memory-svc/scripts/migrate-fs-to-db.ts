#!/usr/bin/env tsx
/**
 * One-time migration: filesystem memory files -> PostgreSQL
 *
 * Reads all .md files from memoryDir, upserts into Postgres (skipping
 * topics with a newer DB updatedAt), generates embeddings for migrated
 * topics that lack them. Rate-limits embedding calls to 10/min.
 *
 * Usage:
 *   pnpm tsx packages/tools/memory-svc/scripts/migrate-fs-to-db.ts
 *
 * Environment:
 *   DATABASE_URL  — required
 *   MEMORY_DIR    — optional, defaults to ~/.nv/memory
 *   OPENAI_API_KEY — optional, enables embedding generation
 */

import { readdir, readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { eq, sql } from "drizzle-orm";
import { db, memory } from "@nova/db";
import pino from "pino";
import { generateEmbedding } from "../src/embedding.js";

const log = pino({
  name: "migrate-fs-to-db",
  transport: { target: "pino-pretty", options: { colorize: true } },
});

const MEMORY_DIR = process.env["MEMORY_DIR"] ?? join(homedir(), ".nv", "memory");
const OPENAI_API_KEY = process.env["OPENAI_API_KEY"];

/** Max embedding calls per minute to avoid OpenAI rate limits */
const EMBEDDING_RATE_LIMIT = 10;
const EMBEDDING_INTERVAL_MS = 60_000;

function sanitizeTopic(filename: string): string {
  // Strip .md extension and convert back to topic (reverse of sanitizeTopic in filesystem.ts)
  return filename.replace(/\.md$/, "");
}

/**
 * Parse frontmatter from a memory file and return content + updatedAt.
 * Handles both files with YAML frontmatter and plain content files.
 */
function parseMemoryFile(raw: string): { content: string; updatedAt: Date; topic: string | null } {
  const frontmatterMatch = raw.match(/^---\n([\s\S]*?)\n---\n([\s\S]*)$/);
  if (frontmatterMatch) {
    const frontmatter = frontmatterMatch[1] ?? "";
    const content = (frontmatterMatch[2] ?? "").trim();

    const updatedMatch = frontmatter.match(/^updated:\s*(.+)$/m);
    const updatedAt = updatedMatch ? new Date(updatedMatch[1]!.trim()) : new Date();

    const topicMatch = frontmatter.match(/^topic:\s*(.+)$/m);
    const topic = topicMatch ? topicMatch[1]!.trim() : null;

    return { content, updatedAt, topic };
  }
  // Plain content file
  return { content: raw.trim(), updatedAt: new Date(), topic: null };
}

async function main(): Promise<void> {
  log.info({ memoryDir: MEMORY_DIR }, "Starting filesystem -> DB migration");

  // List all .md files in memory dir
  let files: string[];
  try {
    const entries = await readdir(MEMORY_DIR);
    files = entries.filter((f) => f.endsWith(".md"));
  } catch (err: unknown) {
    const isNotFound =
      err instanceof Error &&
      "code" in err &&
      (err as NodeJS.ErrnoException).code === "ENOENT";
    if (isNotFound) {
      log.info({ memoryDir: MEMORY_DIR }, "Memory directory does not exist — nothing to migrate");
      return;
    }
    throw err;
  }

  if (files.length === 0) {
    log.info("No .md files found in memory directory — migration complete");
    return;
  }

  log.info({ count: files.length }, `Found ${files.length} memory files to process`);

  let migrated = 0;
  let skipped = 0;
  let errors = 0;
  let embeddingsGenerated = 0;
  let embeddingWindowStart = Date.now();
  let embeddingsThisWindow = 0;

  for (const filename of files) {
    const filePath = join(MEMORY_DIR, filename);
    const topicFromFilename = sanitizeTopic(filename);

    let raw: string;
    try {
      raw = await readFile(filePath, "utf-8");
    } catch (err) {
      log.error({ filename, err }, `Failed to read file — skipping`);
      errors++;
      continue;
    }

    const parsed = parseMemoryFile(raw);
    const topic = parsed.topic ?? topicFromFilename;
    const { content, updatedAt } = parsed;

    if (!content.trim()) {
      log.warn({ filename, topic }, "File has empty content — skipping");
      skipped++;
      continue;
    }

    try {
      // Check if DB has a newer entry
      const existing = await db.query.memory.findFirst({
        where: eq(memory.topic, topic),
        columns: { id: true, updatedAt: true, embedding: true },
      });

      if (existing && existing.updatedAt > updatedAt) {
        log.debug({ topic }, "DB entry is newer — skipping");
        skipped++;
        continue;
      }

      // Check if we need to generate an embedding
      const needsEmbedding = !existing?.embedding;

      let embedding: number[] | null = null;
      if (needsEmbedding && OPENAI_API_KEY) {
        // Rate-limit: max EMBEDDING_RATE_LIMIT per minute
        const now = Date.now();
        if (now - embeddingWindowStart >= EMBEDDING_INTERVAL_MS) {
          embeddingWindowStart = now;
          embeddingsThisWindow = 0;
        }

        if (embeddingsThisWindow >= EMBEDDING_RATE_LIMIT) {
          const waitMs = EMBEDDING_INTERVAL_MS - (now - embeddingWindowStart);
          log.info({ waitMs }, `Rate limit reached — waiting ${Math.ceil(waitMs / 1000)}s`);
          await new Promise<void>((resolve) => setTimeout(resolve, waitMs));
          embeddingWindowStart = Date.now();
          embeddingsThisWindow = 0;
        }

        embedding = await generateEmbedding(
          `${topic}\n\n${content}`,
          OPENAI_API_KEY,
          log as Parameters<typeof generateEmbedding>[2],
        );

        if (embedding) {
          embeddingsThisWindow++;
          embeddingsGenerated++;
        }
      }

      // Upsert into Postgres
      await db
        .insert(memory)
        .values({
          topic,
          content,
          embedding: embedding ?? undefined,
          updatedAt,
        })
        .onConflictDoUpdate({
          target: memory.topic,
          set: {
            content,
            ...(embedding ? { embedding } : {}),
            updatedAt,
          },
        });

      migrated++;
      log.info({ topic, hadEmbedding: !!embedding }, `Migrated: ${topic}`);
    } catch (err) {
      log.error({ topic, err }, `Failed to migrate topic — skipping`);
      errors++;
    }
  }

  log.info(
    { migrated, skipped, errors, embeddingsGenerated },
    `Migration complete: ${migrated} migrated, ${skipped} skipped (newer in DB), ${errors} errors`,
  );

  if (errors > 0) {
    process.exit(1);
  }
}

await main();
