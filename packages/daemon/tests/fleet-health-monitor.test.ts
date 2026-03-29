/**
 * Unit tests for FleetHealthMonitor.
 *
 * Verifies:
 *   - probe() records healthy status when fetch returns 200
 *   - probe() records unhealthy status when fetch rejects (network error)
 *   - probe() records unhealthy status on abort timeout
 *   - getSnapshot() returns current state after probes
 *   - State transitions up->down are detected (logger.error called)
 *   - State transitions down->up are detected (logger.info called)
 *   - Sustained unhealthy state logs at debug level only (no repeat error)
 *   - getSnapshot() returns empty array before first probe
 *
 * HTTP calls are intercepted by replacing globalThis.fetch with controlled mock
 * implementations, matching the pattern used in fleet-client-retry.test.ts.
 * initFleetClient() is called to set the base URL before tests run.
 *
 * Task: add-fleet-health-monitor [4.1]
 */

import { describe, it, before, beforeEach, afterEach, mock } from "node:test";
import assert from "node:assert/strict";

// ── Initialize fleet-client base URL ─────────────────────────────────────────

const { initFleetClient } = await import("../src/fleet-client.js");
// Use non-routable host so any un-intercepted fetch fails fast
initFleetClient("http://localhost:9999");

// ── Import module under test ──────────────────────────────────────────────────

const { FleetHealthMonitor, FLEET_SERVICES } = await import(
  "../src/features/fleet-health/monitor.js"
);

// ── Fetch mock helpers ────────────────────────────────────────────────────────

type FetchMock = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

let originalFetch: typeof globalThis.fetch;

/**
 * Replace globalThis.fetch so every call returns the given factory result.
 * This covers all ports since FleetHealthMonitor probes all FLEET_SERVICES.
 */
function mockFetchAlways(factory: () => Response): void {
  globalThis.fetch = (async () => factory()) as FetchMock;
}

/**
 * Replace globalThis.fetch so it rejects every call with the given error.
 */
function mockFetchReject(error: Error): void {
  globalThis.fetch = (async () => { throw error; }) as FetchMock;
}

function okResponse(): Response {
  return new Response(JSON.stringify({ ok: true }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function statusResponse(status: number): Response {
  return new Response(JSON.stringify({ error: "error" }), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

// ── Setup / teardown ──────────────────────────────────────────────────────────

before(() => {
  originalFetch = globalThis.fetch;
});

afterEach(() => {
  globalThis.fetch = originalFetch;
});

// ── Helpers ───────────────────────────────────────────────────────────────────

function makeConfig(overrides: Partial<{
  enabled: boolean;
  intervalMs: number;
  probeTimeoutMs: number;
  notifyOnCritical: boolean;
}> = {}) {
  return {
    enabled: true,
    intervalMs: 60_000,
    probeTimeoutMs: 3000,
    notifyOnCritical: false,
    ...overrides,
  };
}

function makeLogger() {
  return {
    info:  mock.fn(() => {}),
    warn:  mock.fn(() => {}),
    error: mock.fn(() => {}),
    debug: mock.fn(() => {}),
  };
}

/** Extract log message strings from a pino-style mock: logger.info({...}, "message") */
function logMessages(fn: ReturnType<typeof mock.fn>): string[] {
  return fn.mock.calls.map((c) => {
    const arg = c.arguments[1];
    return typeof arg === "string" ? arg : String(arg ?? "");
  });
}

// ── Tests: getSnapshot() ──────────────────────────────────────────────────────

describe("FleetHealthMonitor.getSnapshot()", () => {
  it("returns empty array before first probe", () => {
    const monitor = new FleetHealthMonitor(makeConfig(), makeLogger() as never);
    assert.deepEqual(monitor.getSnapshot(), []);
  });

  it("returns a shallow copy — mutation does not affect internal state", async () => {
    mockFetchAlways(() => okResponse());
    const monitor = new FleetHealthMonitor(makeConfig(), makeLogger() as never);
    await monitor.probe();

    const snapshot = monitor.getSnapshot();
    snapshot.length = 0; // mutate the returned copy

    const snapshot2 = monitor.getSnapshot();
    assert.equal(snapshot2.length, FLEET_SERVICES.length, "Internal state should be unaffected");
  });
});

// ── Tests: probe() with 200 OK ────────────────────────────────────────────────

describe("FleetHealthMonitor.probe() — service up (200 OK)", () => {
  it("records healthy status for all services", async () => {
    mockFetchAlways(() => okResponse());

    const logger = makeLogger();
    const monitor = new FleetHealthMonitor(makeConfig(), logger as never);
    await monitor.probe();

    const snapshot = monitor.getSnapshot();
    assert.equal(snapshot.length, FLEET_SERVICES.length, "All services should appear in snapshot");

    for (const svc of snapshot) {
      assert.equal(svc.status, "healthy", `${svc.name} should be healthy`);
      assert.ok(typeof svc.latencyMs === "number", `${svc.name} latencyMs should be a number`);
      assert.ok(svc.lastCheckedAt instanceof Date, `${svc.name} lastCheckedAt should be a Date`);
      assert.equal(svc.error, undefined, `${svc.name} should have no error`);
    }
  });

  it("logs startup summary on first probe", async () => {
    mockFetchAlways(() => okResponse());

    const logger = makeLogger();
    const monitor = new FleetHealthMonitor(makeConfig(), logger as never);
    await monitor.probe();

    const msgs = logMessages(logger.info as ReturnType<typeof mock.fn>);
    const summary = msgs.find((m) => m.includes("Fleet health check"));
    assert.ok(summary, "Should log startup summary on first probe");
    assert.ok(
      summary?.includes(`${FLEET_SERVICES.length}/${FLEET_SERVICES.length} healthy`),
      "Startup summary should show all healthy",
    );
  });
});

// ── Tests: probe() with failure ───────────────────────────────────────────────

describe("FleetHealthMonitor.probe() — service down", () => {
  it("records unhealthy status when fetch rejects with a network error", async () => {
    mockFetchReject(new Error("connection refused"));

    const logger = makeLogger();
    const monitor = new FleetHealthMonitor(makeConfig(), logger as never);
    await monitor.probe();

    const snapshot = monitor.getSnapshot();
    for (const svc of snapshot) {
      assert.equal(svc.status, "unhealthy", `${svc.name} should be unhealthy`);
      assert.equal(svc.latencyMs, null, `${svc.name} latencyMs should be null`);
      assert.ok(typeof svc.error === "string", `${svc.name} should have an error string`);
    }
  });

  it("records unhealthy with timeout message when AbortError is thrown", async () => {
    // Simulate an AbortError (timeout) — fleet-client converts this to FleetClientError(504)
    globalThis.fetch = (async (_input: RequestInfo | URL, init?: RequestInit) => {
      init?.signal?.addEventListener("abort", () => {});
      const err = new DOMException("The operation was aborted.", "AbortError");
      throw err;
    }) as FetchMock;

    const logger = makeLogger();
    const monitor = new FleetHealthMonitor(makeConfig({ probeTimeoutMs: 1 }), logger as never);
    await monitor.probe();

    const snapshot = monitor.getSnapshot();
    for (const svc of snapshot) {
      assert.equal(svc.status, "unhealthy");
      assert.ok(typeof svc.error === "string");
    }
  });

  it("records unhealthy when fetch returns a 503 response", async () => {
    mockFetchAlways(() => statusResponse(503));

    const logger = makeLogger();
    const monitor = new FleetHealthMonitor(makeConfig(), logger as never);
    await monitor.probe();

    const snapshot = monitor.getSnapshot();
    for (const svc of snapshot) {
      assert.equal(svc.status, "unhealthy");
      assert.ok(typeof svc.error === "string");
    }
  });

  it("startup summary mentions unhealthy services when some are down", async () => {
    // Make exactly one service fail (tool-router on port 4100) — use port-based discrimination
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      const url = input.toString();
      if (url.includes(":4100/")) return statusResponse(503);
      return okResponse();
    }) as FetchMock;

    const logger = makeLogger();
    const monitor = new FleetHealthMonitor(makeConfig(), logger as never);
    await monitor.probe();

    const msgs = logMessages(logger.info as ReturnType<typeof mock.fn>);
    const summary = msgs.find((m) => m.includes("Fleet health check"));
    assert.ok(summary, "Should log startup summary");
    assert.ok(summary?.includes("unhealthy"), "Summary should mention unhealthy services");
    assert.ok(summary?.includes("tool-router"), "Summary should name the unhealthy service");
  });
});

// ── Tests: state transitions ──────────────────────────────────────────────────

describe("FleetHealthMonitor state transitions", () => {
  it("up->down: logs error for each service that transitioned", async () => {
    // First probe: all healthy
    mockFetchAlways(() => okResponse());
    const logger = makeLogger();
    const monitor = new FleetHealthMonitor(makeConfig(), logger as never);
    await monitor.probe();

    // Second probe: all down
    mockFetchReject(new Error("service crashed"));
    await monitor.probe();

    const errorMessages = logMessages(logger.error as ReturnType<typeof mock.fn>);
    const downLogs = errorMessages.filter((m) => m.includes("Fleet service down"));
    assert.equal(downLogs.length, FLEET_SERVICES.length, "Should log one down entry per service");
  });

  it("down->up: logs info for each service that recovered", async () => {
    // First probe: all down
    mockFetchReject(new Error("unavailable"));
    const logger = makeLogger();
    const monitor = new FleetHealthMonitor(makeConfig(), logger as never);
    await monitor.probe();

    // Second probe: all healthy
    mockFetchAlways(() => okResponse());
    await monitor.probe();

    const infoMessages = logMessages(logger.info as ReturnType<typeof mock.fn>);
    const recoveredLogs = infoMessages.filter((m) => m.includes("Fleet service recovered"));
    assert.equal(recoveredLogs.length, FLEET_SERVICES.length, "Should log one recovered entry per service");
  });

  it("sustained unhealthy: no error logs on 2nd probe, debug logs emitted instead", async () => {
    // First probe: all down
    mockFetchReject(new Error("unavailable"));
    const logger = makeLogger();
    const monitor = new FleetHealthMonitor(makeConfig(), logger as never);
    await monitor.probe();

    // Reset error count — first probe only calls info (startup summary)
    (logger.error as ReturnType<typeof mock.fn>).mock.resetCalls();

    // Second probe: still all down
    mockFetchReject(new Error("still unavailable"));
    await monitor.probe();

    const errorCalls = (logger.error as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(errorCalls.length, 0, "No error logs for sustained unhealthy state");

    const debugCalls = (logger.debug as ReturnType<typeof mock.fn>).mock.calls;
    const stillUnhealthyLogs = debugCalls.filter((c) => {
      const msg = c.arguments[1];
      return typeof msg === "string" && msg.includes("still unhealthy");
    });
    assert.ok(stillUnhealthyLogs.length > 0, "Should log sustained outage at debug level");
  });

  it("healthy->healthy: no info or error logs emitted on 2nd probe", async () => {
    // First probe: all healthy
    mockFetchAlways(() => okResponse());
    const logger = makeLogger();
    const monitor = new FleetHealthMonitor(makeConfig(), logger as never);
    await monitor.probe(); // startup summary is logged here

    // Reset after first probe
    (logger.info as ReturnType<typeof mock.fn>).mock.resetCalls();
    (logger.error as ReturnType<typeof mock.fn>).mock.resetCalls();
    (logger.debug as ReturnType<typeof mock.fn>).mock.resetCalls();

    // Second probe: still all healthy
    mockFetchAlways(() => okResponse());
    await monitor.probe();

    const infoCalls = (logger.info as ReturnType<typeof mock.fn>).mock.calls;
    const errorCalls = (logger.error as ReturnType<typeof mock.fn>).mock.calls;
    assert.equal(infoCalls.length, 0, "No info logs for healthy->healthy transitions");
    assert.equal(errorCalls.length, 0, "No error logs for healthy->healthy transitions");
  });

  it("snapshot reflects the most recent probe result", async () => {
    // First probe: all healthy
    mockFetchAlways(() => okResponse());
    const logger = makeLogger();
    const monitor = new FleetHealthMonitor(makeConfig(), logger as never);
    await monitor.probe();

    assert.ok(monitor.getSnapshot().every((s) => s.status === "healthy"));

    // Second probe: all down
    mockFetchReject(new Error("down"));
    await monitor.probe();

    assert.ok(
      monitor.getSnapshot().every((s) => s.status === "unhealthy"),
      "Snapshot should reflect latest probe result",
    );
  });

  it("lastCheckedAt is updated on each probe", async () => {
    mockFetchAlways(() => okResponse());
    const logger = makeLogger();
    const monitor = new FleetHealthMonitor(makeConfig(), logger as never);

    await monitor.probe();
    const firstTs = monitor.getSnapshot()[0]?.lastCheckedAt;
    assert.ok(firstTs instanceof Date);

    // Small delay so Date.now() advances
    await new Promise((r) => setTimeout(r, 5));

    await monitor.probe();
    const secondTs = monitor.getSnapshot()[0]?.lastCheckedAt;
    assert.ok(secondTs instanceof Date);
    assert.ok(
      secondTs.getTime() >= firstTs.getTime(),
      "lastCheckedAt should advance on each probe",
    );
  });
});

// ── Tests: start() / stop() ───────────────────────────────────────────────────

describe("FleetHealthMonitor.start() / stop()", () => {
  it("stop() before start() is a no-op (does not throw)", () => {
    const monitor = new FleetHealthMonitor(makeConfig(), makeLogger() as never);
    assert.doesNotThrow(() => monitor.stop());
  });

  it("start() triggers an immediate probe and stop() halts the interval", async () => {
    let fetchCallCount = 0;
    globalThis.fetch = (async () => {
      fetchCallCount++;
      return okResponse();
    }) as FetchMock;

    const monitor = new FleetHealthMonitor(
      makeConfig({ intervalMs: 60_000 }),
      makeLogger() as never,
    );

    monitor.start();
    // Allow the fire-and-forget probe to resolve
    await new Promise((r) => setTimeout(r, 50));
    monitor.stop();

    // One full probe pass = FLEET_SERVICES.length fetch calls (fleetGet retries on 5xx,
    // but all return 200 here so exactly one call per service)
    assert.ok(
      fetchCallCount >= FLEET_SERVICES.length,
      `start() should fire an immediate full probe; got ${fetchCallCount} fetch calls`,
    );

    const countAfterStop = fetchCallCount;
    // Verify no further probes fire after stop()
    await new Promise((r) => setTimeout(r, 50));
    assert.equal(fetchCallCount, countAfterStop, "stop() should prevent further probes");
  });
});
