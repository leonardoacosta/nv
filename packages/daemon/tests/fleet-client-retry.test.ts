/**
 * Unit tests for fleet-client retry behavior.
 *
 * Verifies:
 *   - fleetPost/fleetGet retry once on 503, succeed on second attempt
 *   - fleetPost/fleetGet do NOT retry on 4xx — throw immediately
 *   - fleetPost/fleetGet throw FleetClientError after two consecutive 5xx failures
 *
 * HTTP calls are intercepted by replacing globalThis.fetch with controlled mock
 * implementations. initFleetClient() is called to set the base URL before each test.
 *
 * Task: add-fleet-client-retry [4.1]
 */

import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";

// ── Module under test ─────────────────────────────────────────────────────────

const { fleetPost, fleetGet, initFleetClient, FleetClientError } = await import(
  "../src/fleet-client.js"
);

// Use a non-routable host so any un-intercepted fetch fails fast
initFleetClient("http://localhost:9999");

// ── Fetch mock helpers ────────────────────────────────────────────────────────

type FetchMock = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

let originalFetch: typeof globalThis.fetch;

function mockFetchSequence(responses: Array<() => Response>): void {
  let callCount = 0;
  globalThis.fetch = (async () => {
    const factory = responses[callCount];
    callCount++;
    if (!factory) {
      throw new Error(`fetch called more times (${callCount}) than mocked responses (${responses.length})`);
    }
    return factory();
  }) as FetchMock;
}

function okResponse(body: unknown = { ok: true }): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function statusResponse(status: number, body: unknown = { error: "error" }): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

// ── Setup / teardown ──────────────────────────────────────────────────────────

beforeEach(() => {
  originalFetch = globalThis.fetch;
});

afterEach(() => {
  globalThis.fetch = originalFetch;
});

// ── fleetPost tests ───────────────────────────────────────────────────────────

describe("fleetPost retry behavior", () => {
  it("retries once on 503 and succeeds on second attempt", async () => {
    let callCount = 0;
    mockFetchSequence([
      () => { callCount++; return statusResponse(503, { error: "unavailable" }); },
      () => { callCount++; return okResponse({ result: "ok" }); },
    ]);

    const result = await fleetPost(9999, "/tools/test_tool", { input: "x" });

    assert.equal(callCount, 2, "Should have made exactly 2 fetch calls (1 original + 1 retry)");
    assert.deepEqual(result, { result: "ok" });
  });

  it("does NOT retry on 4xx — throws FleetClientError immediately", async () => {
    let callCount = 0;
    mockFetchSequence([
      () => { callCount++; return statusResponse(422, { error: "validation_failed" }); },
    ]);

    await assert.rejects(
      () => fleetPost(9999, "/tools/test_tool", { input: "x" }),
      (err: unknown) => {
        assert.ok(err instanceof FleetClientError, "Should throw FleetClientError");
        assert.equal(err.status, 422, "Error status should be 422");
        return true;
      },
    );

    assert.equal(callCount, 1, "Should have made only 1 fetch call (no retry on 4xx)");
  });

  it("throws FleetClientError after two consecutive 5xx failures", async () => {
    let callCount = 0;
    mockFetchSequence([
      () => { callCount++; return statusResponse(503, { error: "unavailable" }); },
      () => { callCount++; return statusResponse(500, { error: "internal_error" }); },
    ]);

    await assert.rejects(
      () => fleetPost(9999, "/tools/test_tool", { input: "x" }),
      (err: unknown) => {
        assert.ok(err instanceof FleetClientError, "Should throw FleetClientError");
        assert.ok(err.status >= 500, `Expected 5xx status, got ${err.status}`);
        return true;
      },
    );

    assert.equal(callCount, 2, "Should have made exactly 2 fetch calls before throwing");
  });

  it("does NOT retry on 400 Bad Request", async () => {
    let callCount = 0;
    mockFetchSequence([
      () => { callCount++; return statusResponse(400, { error: "bad_request" }); },
    ]);

    await assert.rejects(
      () => fleetPost(9999, "/tools/test_tool", {}),
      (err: unknown) => {
        assert.ok(err instanceof FleetClientError);
        assert.equal(err.status, 400);
        return true;
      },
    );

    assert.equal(callCount, 1, "400 should not be retried");
  });

  it("does NOT retry on 404 Not Found", async () => {
    let callCount = 0;
    mockFetchSequence([
      () => { callCount++; return statusResponse(404, { error: "not_found" }); },
    ]);

    await assert.rejects(
      () => fleetPost(9999, "/tools/test_tool", {}),
      (err: unknown) => {
        assert.ok(err instanceof FleetClientError);
        assert.equal(err.status, 404);
        return true;
      },
    );

    assert.equal(callCount, 1, "404 should not be retried");
  });
});

// ── fleetGet tests ────────────────────────────────────────────────────────────

describe("fleetGet retry behavior", () => {
  it("retries once on 503 and succeeds on second attempt", async () => {
    let callCount = 0;
    mockFetchSequence([
      () => { callCount++; return statusResponse(503, { error: "unavailable" }); },
      () => { callCount++; return okResponse({ data: "value" }); },
    ]);

    const result = await fleetGet(9999, "/status");

    assert.equal(callCount, 2, "Should have made exactly 2 fetch calls (1 original + 1 retry)");
    assert.deepEqual(result, { data: "value" });
  });

  it("does NOT retry on 4xx — throws FleetClientError immediately", async () => {
    let callCount = 0;
    mockFetchSequence([
      () => { callCount++; return statusResponse(403, { error: "forbidden" }); },
    ]);

    await assert.rejects(
      () => fleetGet(9999, "/status"),
      (err: unknown) => {
        assert.ok(err instanceof FleetClientError, "Should throw FleetClientError");
        assert.equal(err.status, 403);
        return true;
      },
    );

    assert.equal(callCount, 1, "Should have made only 1 fetch call (no retry on 4xx)");
  });

  it("throws FleetClientError after two consecutive 5xx failures", async () => {
    let callCount = 0;
    mockFetchSequence([
      () => { callCount++; return statusResponse(500, { error: "internal_error" }); },
      () => { callCount++; return statusResponse(502, { error: "bad_gateway" }); },
    ]);

    await assert.rejects(
      () => fleetGet(9999, "/status"),
      (err: unknown) => {
        assert.ok(err instanceof FleetClientError, "Should throw FleetClientError");
        assert.ok(err.status >= 500, `Expected 5xx status, got ${err.status}`);
        return true;
      },
    );

    assert.equal(callCount, 2, "Should have made exactly 2 fetch calls before throwing");
  });

  it("retries on 500 Internal Server Error and succeeds", async () => {
    let callCount = 0;
    mockFetchSequence([
      () => { callCount++; return statusResponse(500, { error: "transient" }); },
      () => { callCount++; return okResponse({ recovered: true }); },
    ]);

    const result = await fleetGet(9999, "/status");

    assert.equal(callCount, 2, "Should retry on 500");
    assert.deepEqual(result, { recovered: true });
  });

  it("passes custom timeout to the request", async () => {
    // The timeout is applied to AbortController — here we just verify no error
    // is thrown when a custom timeout is provided and the request succeeds
    let callCount = 0;
    mockFetchSequence([
      () => { callCount++; return okResponse({ fast: true }); },
    ]);

    const result = await fleetGet(9999, "/status", 10_000);

    assert.equal(callCount, 1);
    assert.deepEqual(result, { fast: true });
  });
});
