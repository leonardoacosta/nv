/**
 * Fleet fetch helper for calling Hono microservices on the host.
 *
 * Resolves service URLs from environment variables with host.docker.internal defaults.
 * All requests have a 5-second timeout.
 */
declare const FLEET_URLS: Record<string, {
    envVar: string;
    defaultUrl: string;
}>;
export type FleetService = keyof typeof FLEET_URLS;
/**
 * Fetch a fleet service endpoint.
 *
 * @param service - Fleet service name (e.g. "tool-router", "meta-svc")
 * @param path - URL path (e.g. "/health", "/services")
 * @param init - Optional RequestInit overrides
 * @returns The parsed JSON response
 * @throws On network errors or non-OK responses
 */
export declare function fleetFetch<T = unknown>(service: string, path: string, init?: RequestInit): Promise<T>;
export {};
//# sourceMappingURL=fleet.d.ts.map