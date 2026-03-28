/**
 * Fleet fetch helper for calling Hono microservices on the host.
 *
 * Resolves service URLs from environment variables with host.docker.internal defaults.
 * All requests have a 5-second timeout.
 */
const FLEET_URLS = {
    "tool-router": {
        envVar: "TOOL_ROUTER_URL",
        defaultUrl: "http://host.docker.internal:4100",
    },
    "memory-svc": {
        envVar: "MEMORY_SVC_URL",
        defaultUrl: "http://host.docker.internal:4101",
    },
    "messages-svc": {
        envVar: "MESSAGES_SVC_URL",
        defaultUrl: "http://host.docker.internal:4102",
    },
    "channels-svc": {
        envVar: "CHANNELS_SVC_URL",
        defaultUrl: "http://host.docker.internal:4103",
    },
    "meta-svc": {
        envVar: "META_SVC_URL",
        defaultUrl: "http://host.docker.internal:4108",
    },
    daemon: {
        envVar: "DAEMON_URL",
        defaultUrl: "http://host.docker.internal:8400",
    },
};
/**
 * Resolve the base URL for a fleet service.
 */
function resolveUrl(service) {
    const config = FLEET_URLS[service];
    if (!config) {
        throw new Error(`Unknown fleet service: ${service}`);
    }
    return process.env[config.envVar] ?? config.defaultUrl;
}
/**
 * Fetch a fleet service endpoint.
 *
 * @param service - Fleet service name (e.g. "tool-router", "meta-svc")
 * @param path - URL path (e.g. "/health", "/services")
 * @param init - Optional RequestInit overrides
 * @returns The parsed JSON response
 * @throws On network errors or non-OK responses
 */
export async function fleetFetch(service, path, init) {
    const baseUrl = resolveUrl(service);
    const url = `${baseUrl}${path}`;
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 5_000);
    try {
        const response = await fetch(url, {
            ...init,
            signal: controller.signal,
            headers: {
                "Content-Type": "application/json",
                ...init?.headers,
            },
        });
        if (!response.ok) {
            const body = await response.text().catch(() => "");
            throw new Error(`Fleet ${service}${path} returned ${response.status}: ${body}`);
        }
        return (await response.json());
    }
    catch (err) {
        if (err instanceof Error && err.name === "AbortError") {
            throw new Error(`Fleet ${service}${path} timed out after 5s`);
        }
        throw err;
    }
    finally {
        clearTimeout(timeoutId);
    }
}
//# sourceMappingURL=fleet.js.map