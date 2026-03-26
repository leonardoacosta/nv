import { logger } from "../../logger.js";
import { MemoryStore, type MemoryRecord } from "./store.js";
import { MemoryFsSync } from "./fs-sync.js";
import { MemorySearch, type SearchResult } from "./search.js";
import type { Pool } from "pg";

// ─── MemoryService ────────────────────────────────────────────────────────────

export class MemoryService {
  private readonly store: MemoryStore;
  private readonly fsSync: MemoryFsSync;
  private readonly searcher: MemorySearch;

  constructor(store: MemoryStore, fsSync: MemoryFsSync, search: MemorySearch) {
    this.store = store;
    this.fsSync = fsSync;
    this.searcher = search;
  }

  /**
   * Writes to Postgres (source of truth), then syncs to filesystem.
   * Filesystem failure is best-effort: logged but never thrown.
   */
  async upsert(topic: string, content: string): Promise<MemoryRecord> {
    const record = await this.store.upsert(topic, content);

    // Best-effort filesystem sync
    try {
      await this.fsSync.write(topic, content);
    } catch (err) {
      logger.warn({ err, topic }, "Memory fs-sync failed — continuing (Postgres is source of truth)");
    }

    return record;
  }

  /**
   * Returns the memory record for the given topic, or null if not found.
   */
  async get(topic: string): Promise<MemoryRecord | null> {
    return this.store.get(topic);
  }

  /**
   * Returns all topics with their updated_at timestamps.
   */
  async list(): Promise<{ topic: string; updated_at: Date }[]> {
    return this.store.list();
  }

  /**
   * Searches memory. If an embedding vector is provided, performs similarity
   * search via pgvector. Otherwise falls back to keyword (ILIKE) search.
   */
  async search(query: string, embedding?: number[]): Promise<SearchResult[]> {
    if (embedding !== undefined) {
      return this.searcher.bySimilarity(embedding);
    }
    return this.searcher.byKeyword(query);
  }
}

// ─── Factory ──────────────────────────────────────────────────────────────────

/**
 * Creates a fully wired MemoryService from a pg Pool.
 */
export function createMemoryService(pool: Pool): MemoryService {
  const store = new MemoryStore(pool);
  const fsSync = new MemoryFsSync();
  const search = new MemorySearch(pool);
  return new MemoryService(store, fsSync, search);
}
