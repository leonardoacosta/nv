import { Hono } from "hono";
import type { ServiceConfig } from "./config.js";
import type { ToolRegistry } from "./tools.js";
export declare function createHttpApp(registry: ToolRegistry, config: ServiceConfig): Hono;
