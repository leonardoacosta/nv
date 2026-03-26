export interface ServiceConfig {
  serviceName: string;
  servicePort: number;
  logLevel: string;
  corsOrigin: string;
  cloudpcHost: string;
  cloudpcUserPath: string;
}

export function loadConfig(): ServiceConfig {
  return {
    serviceName: process.env["SERVICE_NAME"] ?? "graph-svc",
    servicePort: parseInt(process.env["SERVICE_PORT"] ?? "4007", 10),
    logLevel: process.env["LOG_LEVEL"] ?? "info",
    corsOrigin:
      process.env["CORS_ORIGIN"] ?? "https://nova.leonardoacosta.dev",
    cloudpcHost: process.env["CLOUDPC_HOST"] ?? "cloudpc",
    cloudpcUserPath:
      process.env["CLOUDPC_USER_PATH"] ?? "C:\\Users\\leo.346-CPC-QJXVZ",
  };
}
