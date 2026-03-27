import type { AdapterRegistry } from "./adapters/registry.js";
import type { Logger } from "./logger.js";
export declare function startMcpServer(registry: AdapterRegistry, logger: Logger): Promise<void>;
