export interface MemorySvcConfig {
  serviceName: string;
  port: number;
  databaseUrl: string;
  openaiApiKey: string | undefined;
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
    logLevel: process.env["NV_LOG_LEVEL"] ?? "info",
    corsOrigin:
      process.env["CORS_ORIGIN"] ?? "https://nova.leonardoacosta.dev",
  };
}
