export interface ServiceConfig {
  serviceName: string;
  servicePort: number;
  logLevel: string;
  corsOrigin: string;
  databaseUrl: string;
}

export function loadConfig(): ServiceConfig {
  const databaseUrl = process.env["DATABASE_URL"];
  if (!databaseUrl) {
    throw new Error("DATABASE_URL environment variable is required");
  }

  return {
    serviceName: process.env["SERVICE_NAME"] ?? "messages-svc",
    servicePort: parseInt(process.env["SERVICE_PORT"] ?? "4102", 10),
    logLevel: process.env["LOG_LEVEL"] ?? "info",
    corsOrigin:
      process.env["CORS_ORIGIN"] ?? "https://nova.leonardoacosta.dev",
    databaseUrl,
  };
}
