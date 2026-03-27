import { homedir } from "node:os";
import { join } from "node:path";

export interface MemorySvcConfig {
  serviceName: string;
  port: number;
  databaseUrl: string;
  openaiApiKey: string | undefined;
  memoryDir: string;
  logLevel: string;
  corsOrigin: string;
}

export function loadConfig(): MemorySvcConfig {
  const databaseUrl = process.env["DATABASE_URL"];
  if (!databaseUrl) {
    throw new Error("DATABASE_URL environment variable is required");
  }

  return {
    serviceName: "memory-svc",
    port: parseInt(process.env["PORT"] ?? "4101", 10),
    databaseUrl,
    openaiApiKey: process.env["OPENAI_API_KEY"] ?? undefined,
    memoryDir:
      process.env["MEMORY_DIR"] ?? join(homedir(), ".nv", "memory"),
    logLevel: process.env["NV_LOG_LEVEL"] ?? "info",
    corsOrigin:
      process.env["CORS_ORIGIN"] ?? "https://nova.leonardoacosta.dev",
  };
}
