import { describe, it, beforeEach, afterEach, mock } from "node:test";
import assert from "node:assert/strict";

import { Hono } from "hono";
import { cors } from "hono/cors";
import { secureHeaders } from "hono/secure-headers";
import { DiscordApiError } from "../src/client.js";

// We test the HTTP layer by building a minimal Hono app that mirrors index.ts routes
// but uses mock tool functions instead of a real DiscordClient.

describe("HTTP Server", () => {
  let app: Hono;
  let originalFetch: typeof globalThis.fetch;

  beforeEach(() => {
    originalFetch = globalThis.fetch;
    app = new Hono();
    app.use("*", cors({ origin: "*" }));
    app.use("*", secureHeaders());

    app.onError((err, c) => {
      if (err instanceof DiscordApiError) {
        return c.json(
          { error: err.message },
          err.status as 401 | 403 | 404 | 500,
        );
      }
      return c.json(
        { error: err instanceof Error ? err.message : "Internal Server Error" },
        500,
      );
    });

    // Health endpoint
    app.get("/health", (c) => {
      return c.json({
        status: "ok",
        service: "discord-svc",
        port: 4004,
      });
    });
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  describe("GET /health", () => {
    it("returns 200 with correct JSON structure", async () => {
      const res = await app.request("/health");
      assert.equal(res.status, 200);

      const body = await res.json();
      assert.equal(body.status, "ok");
      assert.equal(body.service, "discord-svc");
      assert.equal(body.port, 4004);
    });

    it("does not include uptime or version (spec: only status, service, port)", async () => {
      const res = await app.request("/health");
      const body = await res.json();

      assert.equal(Object.keys(body).length, 3);
      assert.ok("status" in body);
      assert.ok("service" in body);
      assert.ok("port" in body);
    });
  });
});
