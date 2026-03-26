import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { DiscordClient, DiscordApiError } from "../src/client.js";

describe("DiscordClient", () => {
  it("stores token from constructor", () => {
    const client = new DiscordClient("test-token-123");
    // Client created successfully — no throw
    assert.ok(client);
  });

  it("throws DiscordApiError with status 401 on auth failure", async () => {
    // Mock fetch to return 401
    const originalFetch = globalThis.fetch;
    globalThis.fetch = async () =>
      new Response("Unauthorized", { status: 401 }) as Response;

    try {
      const client = new DiscordClient("bad-token");
      await assert.rejects(
        () => client.get("/users/@me/guilds"),
        (err: unknown) => {
          assert.ok(err instanceof DiscordApiError);
          assert.equal(err.status, 401);
          assert.match(err.message, /auth failed/i);
          return true;
        },
      );
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it("throws DiscordApiError with status 403 for channel permission denied", async () => {
    const originalFetch = globalThis.fetch;
    globalThis.fetch = async () =>
      new Response("Forbidden", { status: 403 }) as Response;

    try {
      const client = new DiscordClient("test-token");
      await assert.rejects(
        () => client.get("/channels/123456789/messages"),
        (err: unknown) => {
          assert.ok(err instanceof DiscordApiError);
          assert.equal(err.status, 403);
          assert.match(err.message, /No permission to read channel 123456789/);
          return true;
        },
      );
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it("throws DiscordApiError with status 404 for guild not found", async () => {
    const originalFetch = globalThis.fetch;
    globalThis.fetch = async () =>
      new Response("Not Found", { status: 404 }) as Response;

    try {
      const client = new DiscordClient("test-token");
      await assert.rejects(
        () => client.get("/guilds/999999999/channels"),
        (err: unknown) => {
          assert.ok(err instanceof DiscordApiError);
          assert.equal(err.status, 404);
          assert.match(err.message, /Guild not found: 999999999/);
          return true;
        },
      );
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it("throws DiscordApiError with status 404 for channel not found", async () => {
    const originalFetch = globalThis.fetch;
    globalThis.fetch = async () =>
      new Response("Not Found", { status: 404 }) as Response;

    try {
      const client = new DiscordClient("test-token");
      await assert.rejects(
        () => client.get("/channels/888888888/messages"),
        (err: unknown) => {
          assert.ok(err instanceof DiscordApiError);
          assert.equal(err.status, 404);
          assert.match(err.message, /Channel not found: 888888888/);
          return true;
        },
      );
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it("retries once on 429 with Retry-After header", async () => {
    const originalFetch = globalThis.fetch;
    let callCount = 0;

    globalThis.fetch = async () => {
      callCount++;
      if (callCount === 1) {
        return new Response("Rate limited", {
          status: 429,
          headers: { "Retry-After": "0.01" },
        }) as Response;
      }
      return new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }) as Response;
    };

    try {
      const client = new DiscordClient("test-token");
      const result = await client.get("/users/@me/guilds");
      assert.deepEqual(result, { ok: true });
      assert.equal(callCount, 2);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});
