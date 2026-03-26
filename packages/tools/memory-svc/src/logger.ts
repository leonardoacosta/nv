import pino, { type Logger, type DestinationStream } from "pino";

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

export interface CreateLoggerOptions {
  level?: string;
  destination?: DestinationStream;
}

export function createLogger(
  name: string,
  options?: CreateLoggerOptions,
): Logger {
  const level = options?.level ?? process.env["NV_LOG_LEVEL"] ?? "info";
  const transport = options?.destination ? undefined : buildTransport();

  return pino(
    {
      name,
      level,
      ...(transport ? { transport } : {}),
    },
    options?.destination,
  );
}

export type { Logger };
