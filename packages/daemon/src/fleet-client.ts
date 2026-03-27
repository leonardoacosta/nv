import { createLogger } from "./logger.js";

const log = createLogger("fleet-client");

const TIMEOUT_MS = 5000;

export class FleetClientError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
    this.name = "FleetClientError";
  }
}

/**
 * Derive the fleet base URL from the tool-router URL.
 * e.g. "http://localhost:4100" -> "http://localhost"
 */
function deriveBaseUrl(toolRouterUrl: string): string {
  try {
    const url = new URL(toolRouterUrl);
    return `${url.protocol}//${url.hostname}`;
  } catch {
    return "http://localhost";
  }
}

let _baseUrl = "http://localhost";

/**
 * Initialize the fleet client with the tool-router URL from config.
 * Must be called once at startup before any fleet calls.
 */
export function initFleetClient(toolRouterUrl: string): void {
  _baseUrl = deriveBaseUrl(toolRouterUrl);
  log.info({ baseUrl: _baseUrl }, "Fleet client initialized");
}

/**
 * GET request to a fleet service.
 */
export async function fleetGet(port: number, path: string, timeoutMs?: number): Promise<unknown> {
  const url = `${_baseUrl}:${port}${path}`;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs ?? TIMEOUT_MS);

  try {
    const res = await fetch(url, {
      method: "GET",
      headers: { "Content-Type": "application/json" },
      signal: controller.signal,
    });

    if (!res.ok) {
      throw new FleetClientError(
        `${res.status} ${res.statusText} from ${url}`,
        res.status,
      );
    }

    return await res.json();
  } catch (err) {
    if (err instanceof FleetClientError) throw err;
    if (err instanceof Error && err.name === "AbortError") {
      throw new FleetClientError(`Timeout after ${TIMEOUT_MS}ms: ${url}`, 504);
    }
    throw new FleetClientError(
      `Fleet request failed: ${url} - ${err instanceof Error ? err.message : String(err)}`,
      503,
    );
  } finally {
    clearTimeout(timer);
  }
}

/**
 * POST request to a fleet service.
 */
export async function fleetPost(
  port: number,
  path: string,
  body: unknown,
): Promise<unknown> {
  const url = `${_baseUrl}:${port}${path}`;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);

  try {
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
      signal: controller.signal,
    });

    if (!res.ok) {
      throw new FleetClientError(
        `${res.status} ${res.statusText} from ${url}`,
        res.status,
      );
    }

    return await res.json();
  } catch (err) {
    if (err instanceof FleetClientError) throw err;
    if (err instanceof Error && err.name === "AbortError") {
      throw new FleetClientError(`Timeout after ${TIMEOUT_MS}ms: ${url}`, 504);
    }
    throw new FleetClientError(
      `Fleet request failed: ${url} - ${err instanceof Error ? err.message : String(err)}`,
      503,
    );
  } finally {
    clearTimeout(timer);
  }
}
