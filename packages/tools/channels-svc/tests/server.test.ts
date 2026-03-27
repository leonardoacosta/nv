import { describe, it, beforeEach } from "node:test";
import assert from "node:assert/strict";

import { AdapterRegistry } from "../src/adapters/registry.js";
import type { ChannelAdapter } from "../src/adapters/registry.js";
import type {
  ChannelName,
  ChannelDirection,
  ChannelStatus,
} from "../src/types.js";
import { createHttpApp } from "../src/server.js";
import { createLogger } from "../src/logger.js";

function createMockAdapter(
  name: ChannelName,
  direction: ChannelDirection = "bidirectional",
  adapterStatus: ChannelStatus = "connected",
  sendFn?: (target: string, message: string) => Promise<void>,
): ChannelAdapter {
  return {
    name,
    direction,
    status: () => adapterStatus,
    send: sendFn ?? (async () => {}),
  };
}

// Silence logs during tests
const logger = createLogger("test", { level: "silent" });

describe("HTTP Server", () => {
  let registry: AdapterRegistry;
  let app: ReturnType<typeof createHttpApp>;

  beforeEach(() => {
    registry = new AdapterRegistry();
    registry.register(createMockAdapter("telegram", "bidirectional", "connected"));
    registry.register(createMockAdapter("discord", "bidirectional", "disconnected"));
    registry.register(createMockAdapter("email", "outbound", "connected"));
    app = createHttpApp(
      registry,
      { serviceName: "channels-svc", servicePort: 4103, corsOrigin: "*" },
      logger,
    );
  });

  describe("GET /health", () => {
    it("returns 200 with service info", async () => {
      const res = await app.request("/health");
      assert.equal(res.status, 200);

      const body = await res.json();
      assert.equal(body.status, "ok");
      assert.equal(body.service, "channels-svc");
      assert.equal(body.port, 4103);
      assert.equal(typeof body.uptime_secs, "number");
    });
  });

  describe("GET /channels", () => {
    it("returns list of registered channels", async () => {
      const res = await app.request("/channels");
      assert.equal(res.status, 200);

      const body = await res.json();
      assert.equal(body.channels.length, 3);

      const telegram = body.channels.find(
        (c: { name: string }) => c.name === "telegram",
      );
      assert.ok(telegram);
      assert.equal(telegram.status, "connected");
      assert.equal(telegram.direction, "bidirectional");
    });
  });

  describe("POST /send", () => {
    it("sends message to connected channel", async () => {
      const res = await app.request("/send", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          channel: "telegram",
          target: "12345",
          message: "hello",
        }),
      });

      assert.equal(res.status, 200);
      const body = await res.json();
      assert.equal(body.ok, true);
      assert.equal(body.channel, "telegram");
      assert.equal(body.target, "12345");
    });

    it("returns 404 for unknown channel", async () => {
      const res = await app.request("/send", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          channel: "sms",
          target: "12345",
          message: "hello",
        }),
      });

      assert.equal(res.status, 404);
      const body = await res.json();
      assert.equal(body.ok, false);
      assert.match(body.error, /not found/i);
    });

    it("returns 503 for disconnected channel", async () => {
      const res = await app.request("/send", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          channel: "discord",
          target: "12345",
          message: "hello",
        }),
      });

      assert.equal(res.status, 503);
      const body = await res.json();
      assert.equal(body.ok, false);
      assert.match(body.error, /disconnected/i);
    });

    it("returns 502 on adapter send failure", async () => {
      const failingRegistry = new AdapterRegistry();
      failingRegistry.register(
        createMockAdapter("telegram", "bidirectional", "connected", async () => {
          throw new Error("Telegram API timeout");
        }),
      );
      const failApp = createHttpApp(
        failingRegistry,
        { serviceName: "channels-svc", servicePort: 4103, corsOrigin: "*" },
        logger,
      );

      const res = await failApp.request("/send", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          channel: "telegram",
          target: "12345",
          message: "hello",
        }),
      });

      assert.equal(res.status, 502);
      const body = await res.json();
      assert.equal(body.ok, false);
      assert.match(body.error, /Telegram API timeout/);
    });

    it("returns 400 for missing required fields", async () => {
      const res = await app.request("/send", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ channel: "telegram" }),
      });

      assert.equal(res.status, 400);
      const body = await res.json();
      assert.equal(body.ok, false);
    });
  });
});
