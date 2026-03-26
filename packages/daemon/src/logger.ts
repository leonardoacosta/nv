import pino, { type Logger } from "pino";

const level = process.env["NV_LOG_LEVEL"] ?? "info";

const isDevelopment = process.env["NODE_ENV"] !== "production";

function buildTransport():
  | { target: string; options: Record<string, unknown> }
  | undefined {
  if (!isDevelopment) return undefined;

  return {
    target: "pino-pretty",
    options: {
      colorize: true,
      translateTime: "SYS:standard",
      ignore: "pid,hostname",
    },
  };
}

export function createLogger(name: string): Logger {
  const transport = buildTransport();
  return pino({
    name,
    level,
    ...(transport ? { transport } : {}),
  });
}

export const logger: Logger = createLogger("nova-daemon");

export type { Logger };
