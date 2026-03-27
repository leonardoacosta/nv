export interface ServiceConfig {
  serviceName: string;
  servicePort: number;
  logLevel: string;
  corsOrigin: string;
  cloudpcHost: string;
}

export function loadConfig(): ServiceConfig {
  return {
    serviceName: process.env["SERVICE_NAME"] ?? "azure-svc",
    servicePort: parseInt(process.env["SERVICE_PORT"] ?? "4109", 10),
    logLevel: process.env["LOG_LEVEL"] ?? "info",
    corsOrigin:
      process.env["CORS_ORIGIN"] ?? "https://nova.leonardoacosta.dev",
    cloudpcHost: process.env["CLOUDPC_HOST"] ?? "cloudpc",
  };
}
