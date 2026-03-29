/**
 * Dynamic tool-to-service registry for the Nova tool fleet.
 *
 * At startup, queries GET /registry on all services listed in nv.toml [tool_router].
 * Builds TOOL_MAP from aggregated responses. Periodic refresh detects changes.
 */

import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import * as TOML from "@iarna/toml";
import pino from "pino";

const log = pino({
  name: "tool-router:registry",
  level: process.env["LOG_LEVEL"] ?? "info",
});

/** Shape returned by each service's GET /registry endpoint */
export interface RegistryResponse {
  service: string;
  tools: ToolDefinition[];
  healthUrl: string;
}

export interface ToolDefinition {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

export interface ServiceEntry {
  serviceUrl: string;
  serviceName: string;
}

/** Unique service list with their base URLs and tool names. */
export interface ServiceInfo {
  serviceName: string;
  serviceUrl: string;
  tools: readonly string[];
  stale?: boolean;
}

/** Config entry from nv.toml [tool_router] */
interface ServiceConfig {
  name: string;
  url: string;
}

interface ToolRouterConfig {
  refresh_interval_s?: number;
  services?: ServiceConfig[];
}

interface TomlWithToolRouter {
  tool_router?: ToolRouterConfig;
}

const DEFAULT_CONFIG_PATH = join(homedir(), ".nv", "config", "nv.toml");
const RETRY_COUNT = 3;
const RETRY_DELAY_MS = 5_000;

/** Current state — replaced atomically on each refresh */
let TOOL_MAP: Map<string, ServiceEntry> = new Map();
let SERVICE_LIST: ServiceInfo[] = [];
let STALE_SERVICES: Set<string> = new Set();

/** Load service URLs from nv.toml [tool_router] */
async function loadServiceConfigs(configPath: string): Promise<ServiceConfig[]> {
  try {
    const raw = await readFile(configPath, "utf-8");
    const toml = TOML.parse(raw) as TomlWithToolRouter;
    return toml.tool_router?.services ?? [];
  } catch (err: unknown) {
    const isNotFound =
      err instanceof Error &&
      "code" in err &&
      (err as NodeJS.ErrnoException).code === "ENOENT";
    if (isNotFound) {
      log.warn({ configPath }, "nv.toml not found — tool-router starting with empty registry");
      return [];
    }
    throw err;
  }
}

/** Load refresh interval from nv.toml [tool_router] */
export async function loadRefreshInterval(
  configPath: string = DEFAULT_CONFIG_PATH,
): Promise<number> {
  try {
    const raw = await readFile(configPath, "utf-8");
    const toml = TOML.parse(raw) as TomlWithToolRouter;
    return (toml.tool_router?.refresh_interval_s ?? 60) * 1000;
  } catch {
    return 60_000;
  }
}

/** Fetch registry from a single service, returning null on failure */
async function fetchRegistry(
  serviceName: string,
  url: string,
): Promise<RegistryResponse | null> {
  try {
    const res = await fetch(`${url}/registry`, {
      signal: AbortSignal.timeout(10_000),
    });
    if (!res.ok) {
      log.warn({ serviceName, url, status: res.status }, "GET /registry returned non-OK status");
      return null;
    }
    const data = (await res.json()) as unknown;
    // Validate response shape
    if (
      typeof data !== "object" ||
      data === null ||
      typeof (data as Record<string, unknown>)["service"] !== "string" ||
      !Array.isArray((data as Record<string, unknown>)["tools"])
    ) {
      log.warn({ serviceName, url }, "GET /registry returned malformed response — skipping");
      return null;
    }
    return data as RegistryResponse;
  } catch (err) {
    log.debug({ serviceName, url, err }, "GET /registry fetch failed");
    return null;
  }
}

/** Fetch with retry — returns null if all attempts fail */
async function fetchRegistryWithRetry(
  serviceName: string,
  url: string,
): Promise<RegistryResponse | null> {
  for (let attempt = 1; attempt <= RETRY_COUNT; attempt++) {
    const result = await fetchRegistry(serviceName, url);
    if (result !== null) return result;

    if (attempt < RETRY_COUNT) {
      log.debug(
        { serviceName, attempt, retryIn: RETRY_DELAY_MS },
        `Retry ${attempt}/${RETRY_COUNT} for ${serviceName} in ${RETRY_DELAY_MS}ms`,
      );
      await new Promise<void>((resolve) => setTimeout(resolve, RETRY_DELAY_MS));
    }
  }
  return null;
}

/**
 * Build a new TOOL_MAP and SERVICE_LIST from registry responses.
 * Returns the new values without mutating global state.
 */
function buildFromResponses(
  configs: ServiceConfig[],
  responses: Map<string, RegistryResponse | null>,
): { toolMap: Map<string, ServiceEntry>; serviceList: ServiceInfo[] } {
  const toolMap = new Map<string, ServiceEntry>();
  const serviceList: ServiceInfo[] = [];

  for (const svc of configs) {
    const registry = responses.get(svc.name);
    if (!registry) continue;

    const toolNames: string[] = [];
    for (const tool of registry.tools) {
      toolMap.set(tool.name, { serviceUrl: svc.url, serviceName: svc.name });
      toolNames.push(tool.name);
    }
    serviceList.push({ serviceName: svc.name, serviceUrl: svc.url, tools: toolNames });
  }

  return { toolMap, serviceList };
}

/**
 * Initialize the registry at startup.
 * Queries all services, retries each 3 times, skips unavailable ones with WARN.
 */
export async function initRegistry(
  configPath: string = DEFAULT_CONFIG_PATH,
): Promise<void> {
  const configs = await loadServiceConfigs(configPath);

  if (configs.length === 0) {
    log.warn("No services configured in [tool_router] — registry is empty");
    return;
  }

  log.info({ services: configs.map((s) => s.name) }, "Initializing registry from services");

  const responses = new Map<string, RegistryResponse | null>();

  // Query all services in parallel with retry
  await Promise.all(
    configs.map(async (svc) => {
      const result = await fetchRegistryWithRetry(svc.name, svc.url);
      if (result === null) {
        log.warn({ service: svc.name, url: svc.url }, `Service ${svc.name} unavailable at startup — skipping`);
      }
      responses.set(svc.name, result);
    }),
  );

  const { toolMap, serviceList } = buildFromResponses(configs, responses);

  // Atomic swap
  TOOL_MAP = toolMap;
  SERVICE_LIST = serviceList;
  STALE_SERVICES = new Set();

  const toolCount = toolMap.size;
  const serviceCount = serviceList.length;
  log.info({ tools: toolCount, services: serviceCount }, `Registered ${toolCount} tools from ${serviceCount} services`);
}

/**
 * Refresh registry from all configured services.
 * Detects added/removed tools, marks stale services, clears stale on recovery.
 * Performs an atomic map swap to avoid race conditions with concurrent dispatch.
 */
export async function refreshRegistry(
  configPath: string = DEFAULT_CONFIG_PATH,
): Promise<void> {
  const configs = await loadServiceConfigs(configPath);

  if (configs.length === 0) return;

  const responses = new Map<string, RegistryResponse | null>();

  await Promise.all(
    configs.map(async (svc) => {
      const result = await fetchRegistry(svc.name, svc.url);
      responses.set(svc.name, result);
    }),
  );

  const newToolMap = new Map<string, ServiceEntry>();
  const newServiceList: ServiceInfo[] = [];
  const newStale = new Set<string>(STALE_SERVICES);

  for (const svc of configs) {
    const registry = responses.get(svc.name);

    if (registry === null || registry === undefined) {
      // Failed refresh — keep last-known tools, mark stale
      const existing = SERVICE_LIST.find((s) => s.serviceName === svc.name);
      if (existing) {
        for (const toolName of existing.tools) {
          const entry = TOOL_MAP.get(toolName);
          if (entry) newToolMap.set(toolName, entry);
        }
        newServiceList.push({ ...existing, stale: true });
      }
      if (!newStale.has(svc.name)) {
        log.warn({ service: svc.name }, `Service ${svc.name} failed refresh — marked stale`);
        newStale.add(svc.name);
      }
      continue;
    }

    const wasStale = newStale.has(svc.name);
    if (wasStale) {
      log.info({ service: svc.name }, `Service ${svc.name} now available (was stale)`);
      newStale.delete(svc.name);
    }

    // Detect added/removed tools
    const prevEntry = SERVICE_LIST.find((s) => s.serviceName === svc.name);
    const prevTools = new Set(prevEntry?.tools ?? []);
    const newTools = new Set(registry.tools.map((t) => t.name));

    for (const t of newTools) {
      if (!prevTools.has(t)) {
        log.info({ service: svc.name, tool: t }, `Service ${svc.name}: added tool ${t}`);
      }
    }
    for (const t of prevTools) {
      if (!newTools.has(t)) {
        log.info({ service: svc.name, tool: t }, `Service ${svc.name}: removed tool ${t}`);
      }
    }

    const toolNames: string[] = [];
    for (const tool of registry.tools) {
      newToolMap.set(tool.name, { serviceUrl: svc.url, serviceName: svc.name });
      toolNames.push(tool.name);
    }
    newServiceList.push({ serviceName: svc.name, serviceUrl: svc.url, tools: toolNames });
  }

  // Atomic swap
  TOOL_MAP = newToolMap;
  SERVICE_LIST = newServiceList;
  STALE_SERVICES = newStale;
}

/**
 * Look up which service handles a given tool name.
 * Returns undefined if the tool is not registered.
 */
export function getServiceForTool(name: string): ServiceEntry | undefined {
  return TOOL_MAP.get(name);
}

/** Return every registered service with its URL and tools list. */
export function getAllServices(): ServiceInfo[] {
  return SERVICE_LIST;
}

/** Return the full flat tool -> service mapping (for /registry endpoint). */
export function getFullRegistry(): Record<string, ServiceEntry> {
  const obj: Record<string, ServiceEntry> = {};
  for (const [tool, entry] of TOOL_MAP) {
    obj[tool] = entry;
  }
  return obj;
}

/** Return names of stale services */
export function getStaleServices(): string[] {
  return Array.from(STALE_SERVICES);
}
