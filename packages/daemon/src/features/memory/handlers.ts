import type { Context } from "hono";
import type { MemoryService } from "./service.js";

// ─── getMemory ────────────────────────────────────────────────────────────────

/**
 * GET /api/memory
 *
 * Behaviour depends on query parameters:
 * - No params           → { topics: string[] }
 * - ?topic=<name>       → { topic: string; content: string } | 404
 * - ?search=<query>     → { results: SearchResult[] }
 *
 * The `getService` parameter is a lazy getter called after validation,
 * allowing tests to verify validation before the service is initialised.
 */
export async function getMemory(
  c: Context,
  getService: () => MemoryService,
): Promise<Response> {
  const topic = c.req.query("topic");
  const search = c.req.query("search");
  const service = getService();

  if (search !== undefined) {
    const results = await service.search(search);
    return c.json({ results });
  }

  if (topic !== undefined) {
    const record = await service.get(topic);
    if (!record) {
      return c.json({ error: `Topic not found: ${topic}` }, 404);
    }
    return c.json({ topic: record.topic, content: record.content });
  }

  // No params — return topic list
  const rows = await service.list();
  const topics = rows.map((r) => r.topic);
  return c.json({ topics });
}

// ─── putMemory ────────────────────────────────────────────────────────────────

/**
 * PUT /api/memory
 *
 * Body: { topic: string; content: string }
 * Response: { topic: string; written: number }
 *
 * The `getService` parameter is a lazy getter called after validation,
 * allowing tests to verify validation before the service is initialised.
 */
export async function putMemory(
  c: Context,
  getService: () => MemoryService,
): Promise<Response> {
  let body: unknown;
  try {
    body = await c.req.json();
  } catch {
    return c.json({ error: "request body must be valid JSON" }, 400);
  }

  if (!body || typeof body !== "object" || Array.isArray(body)) {
    return c.json({ error: "request body must be a JSON object" }, 400);
  }

  const { topic, content } = body as Record<string, unknown>;

  if (!topic || typeof topic !== "string") {
    return c.json({ error: "topic is required and must be a string" }, 400);
  }
  if (content === undefined || content === null || typeof content !== "string") {
    return c.json({ error: "content is required and must be a string" }, 400);
  }

  const service = getService();
  const record = await service.upsert(topic, content);

  return c.json({ topic: record.topic, written: content.length });
}
