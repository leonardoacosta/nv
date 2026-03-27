import { type Logger, type DestinationStream } from "pino";
export interface CreateLoggerOptions {
    level?: string;
    destination?: DestinationStream;
}
export declare function createLogger(name: string, options?: CreateLoggerOptions): Logger;
export type { Logger };
