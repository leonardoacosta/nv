import type { ServiceConfig } from "./config.js";
import type { Logger } from "./logger.js";
export declare function startMcpServer(config: ServiceConfig, logger: Logger): Promise<void>;
