import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { Hono } from "hono";
import type { Logger } from "pino";

import { dispatchRoute } from "./routes/dispatch.js";
import { healthRoute } from "./routes/health.js";
import { registryRoute } from "./routes/registry.js";
import { getFullRegistry } from "./registry.js";

// Minimal pino-compatible logger stub for tests
const noopLogger: Logger = {
  info: () => {},
  warn: () => {},
  error: () => {},
  debug: () => {},
  fatal: () => {},
  trace: () => {},
  child: () => noopLogger,
} as unknown as Logger;

function createTestApp(): Hono {
  const app = new Hono();
  dispatchRoute(app, noopLogger);
  healthRoute(app, noopLogger);
  registryRoute(app);
  return app;
}

// ── Dispatch Tests ──────────────────────────────────────────────────────────

describe("POST /dispatch", () => {
  it("returns 404 for unknown tool", async () => {
    const app = createTestApp();
    const res = await app.request("/dispatch", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ tool: "nonexistent_tool", input: {} }),
    });
    assert.equal(res.status, 404);
    const body = await res.json();
    assert.equal(body.error, "unknown_tool");
    assert.equal(body.tool, "nonexistent_tool");
  });

  it("returns 400 when tool is missing from body", async () => {
    const app = createTestApp();
    const res = await app.request("/dispatch", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ input: {} }),
    });
    assert.equal(res.status, 400);
    const body = await res.json();
    assert.equal(body.error, "missing_tool");
  });

  it("returns 502 when downstream service is unreachable", async () => {
    // calendar_today maps to graph-svc on port 4107.
    // Port 4107 should not have a service running in the test environment.
    const app = createTestApp();
    const res = await app.request("/dispatch", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ tool: "calendar_today", input: {} }),
    });
    // If port 4107 happens to be running, the test still validates the dispatch path
    assert.ok(
      res.status === 502 || res.status === 200,
      `Expected 502 (unreachable) or 200 (service running), got ${res.status}`,
    );
    if (res.status === 502) {
      const body = await res.json();
      assert.equal(body.error, "service_unavailable");
      assert.equal(body.service, "graph-svc");
      assert.equal(body.tool, "calendar_today");
    }
  });
});

// ── Health Tests ────────────────────────────────────────────────────────────

describe("GET /health", () => {
  it("returns valid health response with correct structure", async () => {
    const app = createTestApp();
    const res = await app.request("/health");
    assert.equal(res.status, 200);
    const body = await res.json();

    // Validate response structure
    assert.ok(["healthy", "degraded", "unhealthy"].includes(body.status));
    assert.equal(body.total_count, 8);
    assert.equal(typeof body.healthy_count, "number");
    assert.ok(body.healthy_count >= 0 && body.healthy_count <= 8);

    // Each service entry should have the right shape
    for (const [name, svc] of Object.entries(body.services) as [string, { status: string; url: string; latency_ms: number | null }][]) {
      assert.ok(["healthy", "unreachable"].includes(svc.status), `${name} has invalid status: ${svc.status}`);
      assert.ok(svc.url.startsWith("http://"), `${name} has invalid url: ${svc.url}`);
      if (svc.status === "healthy") {
        assert.equal(typeof svc.latency_ms, "number");
      }
    }

    // Status should match counts
    if (body.healthy_count === body.total_count) {
      assert.equal(body.status, "healthy");
    } else if (body.healthy_count === 0) {
      assert.equal(body.status, "unhealthy");
    } else {
      assert.equal(body.status, "degraded");
    }
  });
});

// ── Registry Tests ──────────────────────────────────────────────────────────

describe("GET /registry", () => {
  it("returns all tool mappings matching the registry", async () => {
    const app = createTestApp();
    const res = await app.request("/registry");
    assert.equal(res.status, 200);
    const body = await res.json();
    const toolNames = Object.keys(body);

    // Validate against the actual registry
    const expectedRegistry = getFullRegistry();
    assert.equal(toolNames.length, Object.keys(expectedRegistry).length);

    // Spot-check a few mappings
    assert.equal(body["read_memory"].serviceName, "memory-svc");
    assert.equal(body["read_memory"].serviceUrl, "http://127.0.0.1:4101");
    assert.equal(body["discord_list_guilds"].serviceName, "discord-svc");
    assert.equal(body["calendar_today"].serviceName, "graph-svc");
    assert.equal(body["update_soul"].serviceName, "meta-svc");
  });
});
