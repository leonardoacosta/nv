import type { Pool } from "pg";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface SearchResult {
  topic: string;
  content: string;
  score?: number;
}

// ─── Row shapes ───────────────────────────────────────────────────────────────

interface KeywordRow {
  topic: string;
  content: string;
}

interface SimilarityRow {
  topic: string;
  content: string;
  distance: number;
}

// ─── MemorySearch ─────────────────────────────────────────────────────────────

export class MemorySearch {
  constructor(private readonly pool: Pool) {}

  /**
   * Full-text style search using SQL ILIKE on both topic and content.
   * Returns up to `limit` results (default 10).
   */
  async byKeyword(query: string, limit = 10): Promise<SearchResult[]> {
    const pattern = `%${query}%`;
    const result = await this.pool.query<KeywordRow>(
      `SELECT topic, content
       FROM memory
       WHERE content ILIKE $1 OR topic ILIKE $1
       LIMIT $2`,
      [pattern, limit],
    );

    return result.rows.map((row) => ({
      topic: row.topic,
      content: row.content,
    }));
  }

  /**
   * Vector similarity search using pgvector <-> (L2 distance) operator.
   * Requires the embedding column to be populated on at least one row.
   * Throws Error("embeddings not configured") if all embedding values are NULL.
   *
   * The caller is responsible for providing the pre-computed embedding vector.
   */
  async bySimilarity(embedding: number[], limit = 10): Promise<SearchResult[]> {
    // Guard: check whether any row has an embedding stored
    const checkResult = await this.pool.query<{ has_embeddings: boolean }>(
      "SELECT EXISTS (SELECT 1 FROM memory WHERE embedding IS NOT NULL) AS has_embeddings",
    );
    const hasEmbeddings = checkResult.rows[0]?.has_embeddings ?? false;
    if (!hasEmbeddings) {
      throw new Error("embeddings not configured");
    }

    // Format the embedding as a Postgres vector literal: [x,x,x,...]
    const vectorLiteral = `[${embedding.join(",")}]`;

    const result = await this.pool.query<SimilarityRow>(
      `SELECT topic, content, embedding <-> $1::vector AS distance
       FROM memory
       WHERE embedding IS NOT NULL
       ORDER BY distance
       LIMIT $2`,
      [vectorLiteral, limit],
    );

    return result.rows.map((row) => ({
      topic: row.topic,
      content: row.content,
      score: row.distance,
    }));
  }
}
