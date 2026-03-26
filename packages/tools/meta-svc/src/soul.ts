import { readFile, writeFile, mkdir } from "node:fs/promises";
import { join, dirname } from "node:path";
import { createLogger } from "./logger.js";

const logger = createLogger("meta-svc");

const SOUL_PATH = join(process.cwd(), "config", "soul.md");

export async function readSoul(): Promise<string> {
  const content = await readFile(SOUL_PATH, "utf-8");
  return content;
}

export async function writeSoul(content: string): Promise<void> {
  const dir = dirname(SOUL_PATH);
  await mkdir(dir, { recursive: true });
  await writeFile(SOUL_PATH, content, "utf-8");
  logger.info(
    { topic: "soul-update", bytes: Buffer.byteLength(content, "utf-8") },
    "Soul document updated",
  );
}
