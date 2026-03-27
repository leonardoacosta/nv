/** Fleet service health check helpers. */
export interface ServiceDef {
    name: string;
    port: number;
    url: string;
}
export interface HealthResult {
    name: string;
    port: number;
    healthy: boolean;
    latencyMs: number | null;
    error?: string;
}
export interface ChannelInfo {
    name: string;
    status: string;
}
/** All 10 fleet services (router + 9 downstream). */
export declare const FLEET_SERVICES: ServiceDef[];
/** Check all fleet services in parallel. */
export declare function checkFleet(timeoutMs?: number): Promise<HealthResult[]>;
/** Fetch channel statuses from channels-svc. */
export declare function getChannels(timeoutMs?: number): Promise<ChannelInfo[]>;
