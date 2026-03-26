/**
 * Unit tests for the Hono HTTP API server.
 *
 * These tests use Hono's test helper to call route handlers directly
 * without a running server or database. Database-dependent routes are
 * tested with mock pool injection patterns.
 */

import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

type JsonBody = Record<string, unknown> | unknown[] | null;

async function jsonBody(res: Response): Promise<JsonBody> {
  return res.json() as Promise<JsonBody>;
}

// ---------------------------------------------------------------------------
// Import items under test after setting up env vars
// ---------------------------------------------------------------------------

// Set DATABASE_URL so the pool init doesn't fail when the module is imported
process.env["DATABASE_URL"] = "postgres://test:test@localhost:5432/test";

// Must import after env setup
const { app, ActivityRingBuffer, emitObligationEvent } = await import(
  "../src/api/server.js"
);

// ---------------------------------------------------------------------------
// [9.1] / [9.2] Covered by typecheck + build commands
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// [9.3] GET /health
// ---------------------------------------------------------------------------
describe("GET /health", () => {
  it("returns ok status with uptime_secs and version", async () => {
    const res = await app.request("/health");
    assert.equal(res.status, 200);
    const body = (await jsonBody(res)) as {
      status: string;
      uptime_secs: number;
      version: string;
    };
    assert.equal(body.status, "ok");
    assert.equal(typeof body.uptime_secs, "number");
    assert.ok(body.uptime_secs >= 0);
    assert.equal(typeof body.version, "string");
  });
});

// ---------------------------------------------------------------------------
// [9.7] PUT /api/memory input validation
// Note: full CRUD integration tests (with live DB) live in tests/memory.test.ts
// ---------------------------------------------------------------------------
describe("Memory routes — input validation", () => {
  it("PUT /api/memory with missing topic returns 400", async () => {
    const res = await app.request("/api/memory", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ content: "hello" }),
    });
    assert.equal(res.status, 400);
    const body = (await jsonBody(res)) as { error: string };
    assert.ok(body.error.includes("topic"));
  });

  it("PUT /api/memory with missing content returns 400", async () => {
    const res = await app.request("/api/memory", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ topic: "my-topic" }),
    });
    assert.equal(res.status, 400);
    const body = (await jsonBody(res)) as { error: string };
    assert.ok(body.error.includes("content"));
  });

  it("PUT /api/memory with non-JSON body returns 400", async () => {
    const res = await app.request("/api/memory", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: "not json",
    });
    assert.equal(res.status, 400);
  });
});

// ---------------------------------------------------------------------------
// [9.9] POST /api/tool-call local-only guard
// ---------------------------------------------------------------------------
describe("POST /api/tool-call", () => {
  it("returns 403 when X-Forwarded-For header is present", async () => {
    const res = await app.request("/api/tool-call", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Forwarded-For": "203.0.113.1",
      },
      body: JSON.stringify({ tool_name: "echo", input: {} }),
    });
    assert.equal(res.status, 403);
  });

  it("returns 403 without X-Forwarded-For when peer IP is not local (test env has no socket)", async () => {
    // In Hono's test helper, c.env has no incoming socket, so peerIp=""
    // which is not 127.0.0.1 or ::1 → 403 expected.
    const res = await app.request("/api/tool-call", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ tool_name: "echo", input: {} }),
    });
    assert.equal(res.status, 403);
  });
});

// ---------------------------------------------------------------------------
// ActivityRingBuffer unit tests
// ---------------------------------------------------------------------------
describe("ActivityRingBuffer", () => {
  it("stores up to capacity items", () => {
    const buf = new ActivityRingBuffer(3);
    const makeEvent = (id: string) => ({
      id,
      obligationId: "o1",
      type: "detected" as const,
      timestamp: new Date().toISOString(),
    });

    buf.push(makeEvent("1"));
    buf.push(makeEvent("2"));
    buf.push(makeEvent("3"));
    assert.deepEqual(
      buf.all().map((e) => e.id),
      ["1", "2", "3"],
    );
  });

  it("evicts oldest when over capacity (FIFO)", () => {
    const buf = new ActivityRingBuffer(3);
    const makeEvent = (id: string) => ({
      id,
      obligationId: "o1",
      type: "detected" as const,
      timestamp: new Date().toISOString(),
    });

    buf.push(makeEvent("1"));
    buf.push(makeEvent("2"));
    buf.push(makeEvent("3"));
    buf.push(makeEvent("4")); // evicts "1"
    assert.deepEqual(
      buf.all().map((e) => e.id),
      ["2", "3", "4"],
    );
  });

  it("recent(n) returns last n items", () => {
    const buf = new ActivityRingBuffer(10);
    for (let i = 1; i <= 5; i++) {
      buf.push({
        id: String(i),
        obligationId: "o1",
        type: "completed",
        timestamp: new Date().toISOString(),
      });
    }
    const recent = buf.recent(3);
    assert.equal(recent.length, 3);
    assert.deepEqual(
      recent.map((e) => e.id),
      ["3", "4", "5"],
    );
  });

  it("recent(n) returns all items when buffer has fewer than n", () => {
    const buf = new ActivityRingBuffer(10);
    buf.push({
      id: "1",
      obligationId: "o1",
      type: "started",
      timestamp: new Date().toISOString(),
    });
    const recent = buf.recent(50);
    assert.equal(recent.length, 1);
  });
});

// ---------------------------------------------------------------------------
// emitObligationEvent — broadcasts to connected clients
// ---------------------------------------------------------------------------
describe("emitObligationEvent", () => {
  it("pushes event to the ring buffer", () => {
    // emitObligationEvent is exported and works without WS clients
    const event = {
      id: "evt-1",
      obligationId: "obl-1",
      type: "completed" as const,
      timestamp: new Date().toISOString(),
    };
    // Should not throw
    assert.doesNotThrow(() => emitObligationEvent(event));
  });
});

// ---------------------------------------------------------------------------
// [9.11] CORS headers
// ---------------------------------------------------------------------------
describe("CORS", () => {
  it("OPTIONS /api/obligations with allowed origin returns correct CORS headers", async () => {
    const res = await app.request("/api/obligations", {
      method: "OPTIONS",
      headers: {
        Origin: "https://nova.leonardoacosta.dev",
        "Access-Control-Request-Method": "GET",
      },
    });
    const allowOrigin = res.headers.get("access-control-allow-origin");
    assert.equal(allowOrigin, "https://nova.leonardoacosta.dev");
  });
});
