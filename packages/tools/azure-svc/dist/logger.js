import pino from "pino";
const isDevelopment = process.env["NODE_ENV"] !== "production";
function buildTransport() {
    if (!isDevelopment)
        return undefined;
    return {
        target: "pino-pretty",
        options: {
            colorize: true,
            translateTime: "SYS:standard",
            ignore: "pid,hostname",
        },
    };
}
export function createLogger(name, options) {
    const level = options?.level ?? process.env["LOG_LEVEL"] ?? "info";
    const transport = options?.destination ? undefined : buildTransport();
    return pino({
        name,
        level,
        ...(transport ? { transport } : {}),
    }, options?.destination);
}
//# sourceMappingURL=logger.js.map