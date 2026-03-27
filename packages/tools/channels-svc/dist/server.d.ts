import { Hono } from "hono";
import type { AdapterRegistry } from "./adapters/registry.js";
import type { Logger } from "./logger.js";
export declare function createHttpApp(registry: AdapterRegistry, config: {
    serviceName: string;
    servicePort: number;
    corsOrigin: string;
}, logger: Logger): Hono;
