import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";

// ─── Helpers ──────────────────────────────────────────────────────────────────

/**
 * Returns the filesystem path to the memory directory.
 * Respects NV_MEMORY_DIR environment variable override.
 */
function getMemoryDir(): string {
  return process.env["NV_MEMORY_DIR"] ?? join(homedir(), ".nv", "memory");
}

/**
 * Strips non-alphanumeric characters except hyphens and underscores,
 * then lowercases the result.
 */
export function sanitizeTopic(topic: string): string {
  return topic.replace(/[^a-zA-Z0-9_-]/g, "").toLowerCase();
}

// ─── MemoryFsSync ─────────────────────────────────────────────────────────────

export class MemoryFsSync {
  private readonly memoryDir: string;

  constructor(memoryDir?: string) {
    this.memoryDir = memoryDir ?? getMemoryDir();
  }

  /**
   * Writes content to ~/.nv/memory/<sanitized-topic>.md.
   * Creates the directory if it does not exist.
   */
  async write(topic: string, content: string): Promise<void> {
    await mkdir(this.memoryDir, { recursive: true });
    const filename = `${sanitizeTopic(topic)}.md`;
    const filePath = join(this.memoryDir, filename);
    await writeFile(filePath, content, "utf-8");
  }

  /**
   * Reads the content of ~/.nv/memory/<sanitized-topic>.md.
   * Returns null if the file does not exist.
   */
  async read(topic: string): Promise<string | null> {
    const filename = `${sanitizeTopic(topic)}.md`;
    const filePath = join(this.memoryDir, filename);
    try {
      return await readFile(filePath, "utf-8");
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") {
        return null;
      }
      throw err;
    }
  }

  /**
   * Lists the stem names (without .md extension) of all memory files.
   * Returns an empty array if the directory does not exist.
   */
  async listTopics(): Promise<string[]> {
    try {
      const entries = await readdir(this.memoryDir);
      return entries
        .filter((f) => f.endsWith(".md"))
        .map((f) => f.slice(0, -3));
    } catch (err) {
      if ((err as NodeJS.ErrnoException).code === "ENOENT") {
        return [];
      }
      throw err;
    }
  }
}
