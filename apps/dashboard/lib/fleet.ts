/**
 * Fleet service fetch helper.
 *
 * Routes dashboard API requests to the appropriate fleet microservice.
 * Service URLs are resolved from environment variables with host.docker.internal
 * defaults (dashboard runs in Docker, fleet services run bare metal on the host).
 */

const SERVICE_URLS: Record<string, string> = {
  "tool-router":
    process.env.TOOL_ROUTER_URL ?? "http://host.docker.internal:4100",
  "memory-svc":
    process.env.MEMORY_SVC_URL ?? "http://host.docker.internal:4101",
  "messages-svc":
    process.env.MESSAGES_SVC_URL ?? "http://host.docker.internal:4102",
  "meta-svc":
    process.env.META_SVC_URL ?? "http://host.docker.internal:4108",
};

/**
 * Fetch a path from a fleet service.
 *
 * @param service - Service name key (e.g. "tool-router", "memory-svc")
 * @param path - Path including leading slash (e.g. "/health")
 * @param init - Standard RequestInit options
 * @returns The raw Response from the fleet service
 * @throws Error if the service name is unknown or the request times out (5s)
 */
export async function fleetFetch(
  service: string,
  path: string,
  init?: RequestInit,
): Promise<Response> {
  const baseUrl = SERVICE_URLS[service];
  if (!baseUrl) {
    throw new Error(`Unknown fleet service: ${service}`);
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 5000);

  try {
    return await fetch(`${baseUrl}${path}`, {
      ...init,
      signal: controller.signal,
    });
  } finally {
    clearTimeout(timeout);
  }
}
