import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

export function sanitizeTopic(topic: string): string {
  return topic
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function buildFrontmatter(topic: string, content: string): string {
  const now = new Date().toISOString();
  const entryCount = content.split("\n").filter((line) => line.startsWith("- ")).length || 1;
  return [
    "---",
    `topic: ${topic}`,
    `created: ${now}`,
    `updated: ${now}`,
    `entries: ${entryCount}`,
    "---",
    "",
    content,
  ].join("\n");
}

export async function writeMemoryFile(
  memoryDir: string,
  topic: string,
  content: string,
): Promise<void> {
  await mkdir(memoryDir, { recursive: true });
  const filename = `${sanitizeTopic(topic)}.md`;
  const filePath = join(memoryDir, filename);
  const fileContent = buildFrontmatter(topic, content);
  await writeFile(filePath, fileContent, "utf-8");
}

export async function readMemoryFile(
  memoryDir: string,
  topic: string,
): Promise<{ content: string; updatedAt: Date } | null> {
  const filename = `${sanitizeTopic(topic)}.md`;
  const filePath = join(memoryDir, filename);

  try {
    const raw = await readFile(filePath, "utf-8");
    // Strip YAML frontmatter
    const match = raw.match(/^---\n[\s\S]*?\n---\n([\s\S]*)$/);
    const content = match ? match[1]!.trim() : raw.trim();
    // Parse updated date from frontmatter
    const updatedMatch = raw.match(/^updated:\s*(.+)$/m);
    const updatedAt = updatedMatch ? new Date(updatedMatch[1]!) : new Date();
    return { content, updatedAt };
  } catch {
    return null;
  }
}
