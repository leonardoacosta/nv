import { readFile } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const TELEGRAM_MAX_CHARS = 4000;
const __dirname = dirname(fileURLToPath(import.meta.url));

/**
 * /soul — read Nova's personality from config/soul.md
 */
export async function buildSoulReply(): Promise<string> {
  // Resolve path: src/telegram/commands -> ../../../../config/soul.md
  // In production (dist/telegram/commands), same relative depth works.
  // Use absolute from project root instead.
  const candidates = [
    join(__dirname, "..", "..", "..", "..", "config", "soul.md"),
    join(__dirname, "..", "..", "..", "config", "soul.md"),
  ];

  for (const path of candidates) {
    try {
      const content = await readFile(path, "utf-8");
      if (content.trim().length === 0) continue;
      return truncate(content.trim());
    } catch {
      // Try next candidate
    }
  }

  return "Soul configuration not found.";
}

function truncate(text: string): string {
  if (text.length <= TELEGRAM_MAX_CHARS) return text;
  return text.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
