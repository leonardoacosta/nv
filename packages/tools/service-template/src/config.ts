export interface ServiceConfig {
  serviceName: string;
  servicePort: number;
  logLevel: string;
  corsOrigin: string;
  databaseUrl?: string;
}

export function loadConfig(): ServiceConfig {
  return {
    serviceName: process.env["SERVICE_NAME"] ?? "service-template",
    servicePort: parseInt(process.env["SERVICE_PORT"] ?? "4000", 10),
    logLevel: process.env["LOG_LEVEL"] ?? "info",
    corsOrigin:
      process.env["CORS_ORIGIN"] ?? "https://nova.leonardoacosta.dev",
    databaseUrl: process.env["DATABASE_URL"] ?? undefined,
  };
}
