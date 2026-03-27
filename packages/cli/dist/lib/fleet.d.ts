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
/** Result of a deep (data-level) service check. */
export interface DeepCheckResult {
    /** Display label (service name or sub-check like "graph-svc/calendar"). */
    label: string;
    port: number;
    /** "ok" = working with data, "empty" = works but no data, "error" = broken. */
    status: "ok" | "empty" | "error";
    /** Human-readable detail (e.g. "16 topics, 31KB total"). */
    detail: string;
    latencyMs: number;
}
/** All 10 fleet services (router + 9 downstream). */
export declare const FLEET_SERVICES: ServiceDef[];
/** Check all fleet services in parallel. */
export declare function checkFleet(timeoutMs?: number): Promise<HealthResult[]>;
/** Fetch channel statuses from channels-svc. */
export declare function getChannels(timeoutMs?: number): Promise<ChannelInfo[]>;
/** Deep-check all fleet services for meaningful data. */
export declare function checkFleetDeep(timeoutMs?: number): Promise<DeepCheckResult[]>;
