/** Fleet service health check helpers. */
/** All 10 fleet services (router + 9 downstream). */
export const FLEET_SERVICES = [
    { name: "tool-router", port: 4100, url: "http://127.0.0.1:4100" },
    { name: "memory-svc", port: 4101, url: "http://127.0.0.1:4101" },
    { name: "messages-svc", port: 4102, url: "http://127.0.0.1:4102" },
    { name: "channels-svc", port: 4103, url: "http://127.0.0.1:4103" },
    { name: "discord-svc", port: 4104, url: "http://127.0.0.1:4104" },
    { name: "teams-svc", port: 4105, url: "http://127.0.0.1:4105" },
    { name: "schedule-svc", port: 4106, url: "http://127.0.0.1:4106" },
    { name: "graph-svc", port: 4107, url: "http://127.0.0.1:4107" },
    { name: "meta-svc", port: 4108, url: "http://127.0.0.1:4108" },
    { name: "azure-svc", port: 4109, url: "http://127.0.0.1:4109" },
];
/** Check a single service's /health endpoint. */
async function checkService(svc, timeoutMs) {
    const start = performance.now();
    try {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), timeoutMs);
        const res = await fetch(`${svc.url}/health`, { signal: controller.signal });
        clearTimeout(timer);
        const latencyMs = Math.round(performance.now() - start);
        return {
            name: svc.name,
            port: svc.port,
            healthy: res.ok,
            latencyMs,
        };
    }
    catch (err) {
        return {
            name: svc.name,
            port: svc.port,
            healthy: false,
            latencyMs: null,
            error: err instanceof Error ? err.message : String(err),
        };
    }
}
/** Check all fleet services in parallel. */
export async function checkFleet(timeoutMs = 3000) {
    return Promise.all(FLEET_SERVICES.map((svc) => checkService(svc, timeoutMs)));
}
/** Fetch channel statuses from channels-svc. */
export async function getChannels(timeoutMs = 3000) {
    try {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), timeoutMs);
        const res = await fetch("http://127.0.0.1:4103/channels", {
            signal: controller.signal,
        });
        clearTimeout(timer);
        if (!res.ok)
            return [];
        const data = (await res.json());
        return data.channels ?? [];
    }
    catch {
        return [];
    }
}
// ---------------------------------------------------------------------------
// Deep check helpers — verify meaningful data from each fleet service
// ---------------------------------------------------------------------------
/** Fetch JSON from a URL with timeout. Returns parsed body or throws. */
async function fetchJson(url, timeoutMs, init) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    try {
        const res = await fetch(url, { ...init, signal: controller.signal });
        if (!res.ok)
            throw new Error(`HTTP ${res.status}`);
        return (await res.json());
    }
    finally {
        clearTimeout(timer);
    }
}
/** POST JSON to a URL with timeout. Returns parsed body or throws. */
async function postJson(url, body, timeoutMs) {
    return fetchJson(url, timeoutMs, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
    });
}
/** Run a single deep check, returning a DeepCheckResult. Never throws. */
async function runDeep(label, port, fn, timeoutMs) {
    const start = performance.now();
    try {
        const result = await fn();
        return { label, port, ...result, latencyMs: Math.round(performance.now() - start) };
    }
    catch (err) {
        const detail = err instanceof Error ? err.message : String(err);
        return { label, port, status: "error", detail, latencyMs: Math.round(performance.now() - start) };
    }
}
/** Format byte count to human-readable (KB/MB). */
function formatBytes(bytes) {
    if (bytes < 1024)
        return `${bytes}B`;
    if (bytes < 1024 * 1024)
        return `${(bytes / 1024).toFixed(1)}KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}
/** Deep-check all fleet services for meaningful data. */
export async function checkFleetDeep(timeoutMs = 15_000) {
    const sshTimeoutMs = 30_000; // SSH-backed services need more time
    const results = await Promise.allSettled([
        // tool-router :4100 — GET /health and count registered services
        runDeep("tool-router", 4100, async () => {
            const data = await fetchJson("http://127.0.0.1:4100/health", timeoutMs);
            const count = Array.isArray(data.services) ? data.services.length : null;
            return count !== null
                ? { status: "ok", detail: `healthy (${count} services)` }
                : { status: "ok", detail: "healthy" };
        }, timeoutMs),
        // memory-svc :4101 — POST /read returns {topic, content} for a single topic
        runDeep("memory-svc", 4101, async () => {
            const data = await postJson("http://127.0.0.1:4101/read", { topic: "architecture" }, timeoutMs);
            if (data.content && data.content.length > 0) {
                return { status: "ok", detail: `topic readable (${(data.content.length / 1024).toFixed(1)}KB)` };
            }
            return { status: "empty", detail: "no topics found" };
        }, timeoutMs),
        // messages-svc :4102 — GET /recent returns {result: [...], error}
        runDeep("messages-svc", 4102, async () => {
            const data = await fetchJson("http://127.0.0.1:4102/recent?limit=1", timeoutMs);
            const msgs = Array.isArray(data.result) ? data.result : [];
            return msgs.length > 0
                ? { status: "ok", detail: "messages accessible" }
                : { status: "empty", detail: "no messages found" };
        }, timeoutMs),
        // channels-svc :4103 — GET /channels
        runDeep("channels-svc", 4103, async () => {
            const data = await fetchJson("http://127.0.0.1:4103/channels", timeoutMs);
            const channels = Array.isArray(data.channels) ? data.channels : [];
            if (channels.length === 0)
                return { status: "empty", detail: "no channels configured" };
            const connected = channels.filter((c) => c.status === "connected").length;
            return { status: connected > 0 ? "ok" : "empty", detail: `${connected}/${channels.length} channels connected` };
        }, timeoutMs),
        // discord-svc :4104 — GET /guilds
        runDeep("discord-svc", 4104, async () => {
            const data = await fetchJson("http://127.0.0.1:4104/guilds", timeoutMs);
            const guilds = Array.isArray(data.guilds) ? data.guilds : [];
            return guilds.length > 0
                ? { status: "ok", detail: `${guilds.length} guilds` }
                : { status: "empty", detail: "0 guilds (bot not invited)" };
        }, timeoutMs),
        // teams-svc :4105 — GET /chats returns {ok, data: string} (SSH text output)
        runDeep("teams-svc", 4105, async () => {
            const data = await fetchJson("http://127.0.0.1:4105/chats", sshTimeoutMs);
            if (data.ok && data.data && data.data.length > 50) {
                const chatCount = (data.data.match(/ID:/g) || []).length;
                return { status: "ok", detail: `${chatCount} chats loaded` };
            }
            return { status: "empty", detail: "no chats (SSH timeout?)" };
        }, sshTimeoutMs),
        // schedule-svc :4106 — GET /health (no data endpoint needed)
        runDeep("schedule-svc", 4106, async () => {
            await fetchJson("http://127.0.0.1:4106/health", timeoutMs);
            return { status: "ok", detail: "healthy" };
        }, timeoutMs),
        // graph-svc :4107 — GET /calendar/today (SSH-backed)
        runDeep("graph-svc/calendar", 4107, async () => {
            const data = await fetchJson("http://127.0.0.1:4107/calendar/today", sshTimeoutMs);
            // Response may be {result: "JSON string"} or {events: [...]}
            let events = [];
            if (typeof data.result === "string") {
                try {
                    events = JSON.parse(data.result);
                }
                catch { /* fall through */ }
                if (!Array.isArray(events)) {
                    try {
                        const parsed = JSON.parse(data.result);
                        events = parsed.value ?? [];
                    }
                    catch {
                        events = [];
                    }
                }
            }
            else if (Array.isArray(data.events)) {
                events = data.events;
            }
            return { status: "ok", detail: `calendar: ${events.length} events today` };
        }, sshTimeoutMs),
        // graph-svc :4107 — GET /pim/status (SSH-backed)
        runDeep("graph-svc/pim", 4107, async () => {
            const data = await fetchJson("http://127.0.0.1:4107/pim/status", sshTimeoutMs);
            // Response may be {result: "JSON string"} or {roles: [...]}
            let roles = [];
            if (typeof data.result === "string") {
                try {
                    const parsed = JSON.parse(data.result);
                    roles = parsed.value ?? [];
                }
                catch {
                    roles = [];
                }
            }
            else if (Array.isArray(data.roles)) {
                roles = data.roles;
            }
            return roles.length > 0
                ? { status: "ok", detail: `pim: ${roles.length} eligible roles` }
                : { status: "empty", detail: "pim: no eligible roles (auth issue?)" };
        }, sshTimeoutMs),
        // graph-svc :4107 — GET /ado/projects returns {result: "JSON string"}
        runDeep("graph-svc/ado", 4107, async () => {
            const data = await fetchJson("http://127.0.0.1:4107/ado/projects", sshTimeoutMs);
            if (data.result) {
                try {
                    const parsed = JSON.parse(data.result);
                    const count = parsed.value?.length ?? parsed.count ?? 0;
                    return count > 0
                        ? { status: "ok", detail: `ado: ${count} projects` }
                        : { status: "empty", detail: "ado: no projects (SSH issue?)" };
                }
                catch { /* fall through */ }
            }
            return { status: "empty", detail: "ado: no projects (SSH issue?)" };
        }, sshTimeoutMs),
        // meta-svc :4108 — GET /health (simpler than /services which needs JSON.parse)
        runDeep("meta-svc", 4108, async () => {
            const data = await fetchJson("http://127.0.0.1:4108/health", timeoutMs);
            // /health returns basic status; if we get a response, the service is up
            if (data.status === "ok" || data.result) {
                return { status: "ok", detail: "healthy" };
            }
            return { status: "ok", detail: "healthy" };
        }, timeoutMs),
        // azure-svc :4109 — POST /az with az account show (SSH-backed)
        runDeep("azure-svc", 4109, async () => {
            const data = await postJson("http://127.0.0.1:4109/az", { command: "az account show" }, sshTimeoutMs);
            // The response might contain the account info directly or in an output field
            const state = data.state ?? "";
            const name = data.name ?? "";
            if (state === "Enabled" || name) {
                return { status: "ok", detail: `az: subscription active${name ? ` (${name})` : ""}` };
            }
            // Try parsing output as JSON if it's a stringified response
            if (data.output) {
                try {
                    const parsed = JSON.parse(data.output);
                    if (parsed.state === "Enabled" || parsed.name) {
                        return { status: "ok", detail: `az: subscription active${parsed.name ? ` (${parsed.name})` : ""}` };
                    }
                }
                catch {
                    // output is not JSON — treat non-empty as success
                    if (data.output.trim())
                        return { status: "ok", detail: "az: subscription active" };
                }
            }
            return { status: "empty", detail: "az: no subscription info" };
        }, sshTimeoutMs),
    ]);
    // Unwrap Promise.allSettled — rejected promises become error results
    return results.map((r, i) => {
        if (r.status === "fulfilled")
            return r.value;
        const labels = [
            "tool-router", "memory-svc", "messages-svc", "channels-svc",
            "discord-svc", "teams-svc", "schedule-svc", "graph-svc/calendar",
            "graph-svc/pim", "graph-svc/ado", "meta-svc", "azure-svc",
        ];
        const ports = [4100, 4101, 4102, 4103, 4104, 4105, 4106, 4107, 4107, 4107, 4108, 4109];
        return {
            label: labels[i] ?? "unknown",
            port: ports[i] ?? 0,
            status: "error",
            detail: r.reason instanceof Error ? r.reason.message : String(r.reason),
            latencyMs: 0,
        };
    });
}
//# sourceMappingURL=fleet.js.map