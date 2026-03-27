/**
 * Tier 2: Embedding-based message router.
 * Uses local all-MiniLM-L6-v2 via @xenova/transformers for semantic similarity.
 * Compares incoming text against pre-computed intent centroids.
 * Falls back gracefully if model loading fails.
 */

import { readdir, readFile, writeFile } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createLogger } from "../logger.js";

const log = createLogger("embedding-router");

const __dirname = dirname(fileURLToPath(import.meta.url));
const INTENTS_DIR = join(__dirname, "..", "..", "src", "brain", "intents");

const DEFAULT_THRESHOLD = 0.82;

export interface EmbeddingMatch {
  tool: string;
  port: number;
  confidence: number;
}

interface IntentData {
  tool: string;
  port: number;
  utterances: string[];
  centroid?: number[];
}

interface LoadedIntent {
  tool: string;
  port: number;
  centroid: number[];
  filePath: string;
}

type PipelineFn = (texts: string | string[], options?: { pooling: string; normalize: boolean }) => Promise<{ data: Float32Array; dims: number[] }>;

export class EmbeddingRouter {
  private intents: LoadedIntent[];
  private pipeline: PipelineFn;
  private threshold: number;

  private constructor(
    intents: LoadedIntent[],
    pipeline: PipelineFn,
    threshold: number,
  ) {
    this.intents = intents;
    this.pipeline = pipeline;
    this.threshold = threshold;
  }

  /**
   * Factory method that loads the model and intent centroids.
   * Returns null if model loading fails (Tier 2 disabled gracefully).
   */
  static async create(): Promise<EmbeddingRouter | null> {
    const threshold = parseFloat(process.env["NV_EMBEDDING_THRESHOLD"] ?? "") || DEFAULT_THRESHOLD;

    let pipeline: PipelineFn;
    try {
      // Dynamic import to avoid hard failure if @xenova/transformers is missing
      const { pipeline: createPipeline } = await import("@xenova/transformers");
      log.info("Loading embedding model (Xenova/all-MiniLM-L6-v2)...");
      const extractor = await createPipeline(
        "feature-extraction",
        "Xenova/all-MiniLM-L6-v2",
      );
      pipeline = extractor as unknown as PipelineFn;
      log.info("Embedding model loaded successfully");
    } catch (err: unknown) {
      log.warn(
        { err: err instanceof Error ? err.message : String(err) },
        "Failed to load embedding model — Tier 2 disabled",
      );
      return null;
    }

    // Load intent files
    let intentFiles: string[];
    try {
      const dirEntries = await readdir(INTENTS_DIR);
      intentFiles = dirEntries.filter((f) => f.endsWith(".json"));
    } catch (err: unknown) {
      log.warn(
        { err: err instanceof Error ? err.message : String(err), dir: INTENTS_DIR },
        "Failed to read intents directory — Tier 2 disabled",
      );
      return null;
    }

    const loadedIntents: LoadedIntent[] = [];

    for (const file of intentFiles) {
      const filePath = join(INTENTS_DIR, file);
      try {
        const raw = await readFile(filePath, "utf-8");
        const data = JSON.parse(raw) as IntentData;

        let centroid: number[];

        if (data.centroid && data.centroid.length > 0) {
          centroid = data.centroid;
          log.debug({ tool: data.tool }, "Loaded cached centroid");
        } else {
          // Compute centroid from utterances
          log.info({ tool: data.tool, utterances: data.utterances.length }, "Computing centroid...");
          centroid = await computeCentroid(pipeline, data.utterances);

          // Cache centroid back to file
          const updated: IntentData = { ...data, centroid };
          await writeFile(filePath, JSON.stringify(updated, null, 2) + "\n", "utf-8");
          log.info({ tool: data.tool }, "Centroid computed and cached");
        }

        loadedIntents.push({
          tool: data.tool,
          port: data.port,
          centroid,
          filePath,
        });
      } catch (err: unknown) {
        log.warn(
          { err: err instanceof Error ? err.message : String(err), file },
          "Failed to load intent file — skipping",
        );
      }
    }

    log.info(
      { intentsLoaded: loadedIntents.length, threshold },
      "Embedding router ready",
    );

    return new EmbeddingRouter(loadedIntents, pipeline, threshold);
  }

  /**
   * Encode the input text and find the closest intent centroid.
   * Returns the best match if similarity >= threshold, null otherwise.
   */
  async match(text: string): Promise<EmbeddingMatch | null> {
    const embedding = await encode(this.pipeline, text);

    let bestSimilarity = -1;
    let bestIntent: LoadedIntent | null = null;

    for (const intent of this.intents) {
      const similarity = cosineSimilarity(embedding, intent.centroid);
      if (similarity > bestSimilarity) {
        bestSimilarity = similarity;
        bestIntent = intent;
      }
    }

    if (bestIntent && bestSimilarity >= this.threshold) {
      return {
        tool: bestIntent.tool,
        port: bestIntent.port,
        confidence: bestSimilarity,
      };
    }

    return null;
  }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

async function encode(pipeline: PipelineFn, text: string): Promise<number[]> {
  const output = await pipeline(text, { pooling: "mean", normalize: true });
  return Array.from(output.data);
}

async function computeCentroid(
  pipeline: PipelineFn,
  utterances: string[],
): Promise<number[]> {
  const embeddings: number[][] = [];

  for (const utterance of utterances) {
    const emb = await encode(pipeline, utterance);
    embeddings.push(emb);
  }

  if (embeddings.length === 0) return [];

  const dim = embeddings[0].length;
  const centroid = new Array<number>(dim).fill(0);

  for (const emb of embeddings) {
    for (let i = 0; i < dim; i++) {
      centroid[i] += emb[i];
    }
  }

  for (let i = 0; i < dim; i++) {
    centroid[i] /= embeddings.length;
  }

  // Normalize the centroid
  const norm = Math.sqrt(centroid.reduce((sum, v) => sum + v * v, 0));
  if (norm > 0) {
    for (let i = 0; i < dim; i++) {
      centroid[i] /= norm;
    }
  }

  return centroid;
}

function cosineSimilarity(a: number[], b: number[]): number {
  let dot = 0;
  let normA = 0;
  let normB = 0;

  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }

  const denom = Math.sqrt(normA) * Math.sqrt(normB);
  return denom === 0 ? 0 : dot / denom;
}
