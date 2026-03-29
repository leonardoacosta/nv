/**
 * Tests for dynamic tool-to-service registry.
 *
 * Covers:
 *  4.1 - GET /registry shape validation
 *  4.2 - initRegistry populates TOOL_MAP; getServiceForTool() resolves
 *  4.3 - periodic refresh detects newly added tool
 *  4.4 - stale service handling: tools retained on failure, stale cleared on recovery
 */

import { describe, it, mock, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import * as TOML from "@iarna/toml";

// ── Helpers ───────────────────────────────────────────────────────────────────

let tmpDir: string;
let configPath: string;

async function writeConfig(services: Array<{ name: string; url: string }>, refreshIntervalS = 60): Promise<void> {
  const toml = TOML.stringify({
    tool_router: {
      refresh_interval_s: refreshIntervalS,
      services,
    },
  } as Parameters<typeof TOML.stringify>[0]);
  await writeFile(configPath, toml, "utf-8");
}

// Simple mock HTTP server using node:http
import { createServer } from "node:http";
import type { Server } from "node:http";

interface MockService {
  server: Server;
  port: number;
  url: string;
  /** Replace the tools returned by /registry */
  setTools: (tools: Array<{ name: string; description: string; inputSchema: Record<string, unknown> }>) => void;
  /** Make /registry return non-OK */
  setDown: (down: boolean) => void;
}

async function startMockService(
  serviceName: string,
  initialTools: Array<{ name: string; description: string; inputSchema: Record<string, unknown> }>,
): Promise<MockService> {
  let tools = [...initialTools];
  let isDown = false;

  const server = createServer((req, res) => {
    if (req.url === "/registry") {
      if (isDown) {
        res.writeHead(503);
        res.end("Service Unavailable");
        return;
      }
      const body = JSON.stringify({
        service: serviceName,
        tools,
        healthUrl: `http://127.0.0.1:0/health`,
      });
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(body);
      return;
    }
    res.writeHead(404);
    res.end();
  });

  const port = await new Promise<number>((resolve, reject) => {
    server.listen(0, "127.0.0.1", () => {
      const addr = server.address();
      if (!addr || typeof addr === "string") {
        reject(new Error("Unexpected address format"));
        return;
      }
      resolve(addr.port);
    });
  });

  return {
    server,
    port,
    url: `http://127.0.0.1:${port}`,
    setTools: (newTools) => { tools = newTools; },
    setDown: (down) => { isDown = down; },
  };
}

function closeService(svc: MockService): Promise<void> {
  return new Promise((resolve, reject) => {
    svc.server.close((err) => (err ? reject(err) : resolve()));
  });
}

// ── Setup/Teardown ────────────────────────────────────────────────────────────

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "nv-registry-test-"));
  configPath = join(tmpDir, "nv.toml");
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

// ── 4.1: GET /registry shape ──────────────────────────────────────────────────

describe("GET /registry shape", () => {
  it("mock service exposes correct shape: service, tools[], healthUrl", async () => {
    const svc = await startMockService("memory-svc", [
      {
        name: "read_memory",
        description: "Read a memory file by topic name.",
        inputSchema: {
          type: "object",
          properties: { topic: { type: "string" } },
          required: ["topic"],
        },
      },
    ]);

    try {
      const res = await fetch(`${svc.url}/registry`);
      assert.equal(res.status, 200);

      const body = await res.json() as Record<string, unknown>;

      // Top-level keys
      assert.equal(typeof body["service"], "string", "service must be a string");
      assert.ok(Array.isArray(body["tools"]), "tools must be an array");
      assert.equal(typeof body["healthUrl"], "string", "healthUrl must be a string");

      // Tool shape
      const tools = body["tools"] as Array<Record<string, unknown>>;
      assert.ok(tools.length > 0, "tools array must not be empty");
      const tool = tools[0]!;
      assert.equal(typeof tool["name"], "string", "tool.name must be a string");
      assert.equal(typeof tool["description"], "string", "tool.description must be a string");
      assert.equal(typeof tool["inputSchema"], "object", "tool.inputSchema must be an object");
      assert.ok(tool["inputSchema"] !== null, "tool.inputSchema must not be null");

      // Values match
      assert.equal(body["service"], "memory-svc");
      assert.equal(tool["name"], "read_memory");
    } finally {
      await closeService(svc);
    }
  });

  it("GET /registry returns 503 when service is down", async () => {
    const svc = await startMockService("test-svc", []);
    svc.setDown(true);

    try {
      const res = await fetch(`${svc.url}/registry`);
      assert.equal(res.status, 503);
    } finally {
      await closeService(svc);
    }
  });
});

// ── 4.2: initRegistry populates TOOL_MAP ─────────────────────────────────────

describe("initRegistry + getServiceForTool", () => {
  it("registers tools from available services; getServiceForTool() resolves correctly", async () => {
    const svc = await startMockService("memory-svc", [
      {
        name: "read_memory",
        description: "Read memory",
        inputSchema: { type: "object", properties: {}, required: [] },
      },
      {
        name: "write_memory",
        description: "Write memory",
        inputSchema: { type: "object", properties: {}, required: [] },
      },
    ]);

    try {
      await writeConfig([{ name: "memory-svc", url: svc.url }]);

      const { initRegistry, getServiceForTool, getAllServices } = await import("./registry.js");

      // Reset module state by re-calling initRegistry
      await initRegistry(configPath);

      const readEntry = getServiceForTool("read_memory");
      assert.ok(readEntry !== undefined, "read_memory must be registered");
      assert.equal(readEntry!.serviceName, "memory-svc");
      assert.equal(readEntry!.serviceUrl, svc.url);

      const writeEntry = getServiceForTool("write_memory");
      assert.ok(writeEntry !== undefined, "write_memory must be registered");
      assert.equal(writeEntry!.serviceName, "memory-svc");

      const services = getAllServices();
      assert.ok(services.length >= 1, "at least one service should be registered");
      const memSvc = services.find((s) => s.serviceName === "memory-svc");
      assert.ok(memSvc !== undefined, "memory-svc must appear in getAllServices()");
      assert.ok(
        (memSvc!.tools as readonly string[]).includes("read_memory"),
        "read_memory must be in memory-svc.tools",
      );
    } finally {
      await closeService(svc);
    }
  });

  it("unknown tool returns undefined from getServiceForTool()", async () => {
    await writeConfig([]);

    const { initRegistry, getServiceForTool } = await import("./registry.js");
    await initRegistry(configPath);

    const entry = getServiceForTool("nonexistent_tool_xyz");
    assert.equal(entry, undefined);
  });

  it("unavailable service at startup is skipped gracefully", async () => {
    // Write config pointing to a port that has nothing listening
    await writeConfig([{ name: "dead-svc", url: "http://127.0.0.1:19999" }]);

    const { initRegistry, getServiceForTool } = await import("./registry.js");

    // Should not throw even when service is unreachable
    await assert.doesNotReject(() => initRegistry(configPath));

    const entry = getServiceForTool("some_dead_tool");
    assert.equal(entry, undefined, "tools from unavailable services must not be registered");
  });
});

// ── 4.3: periodic refresh detects newly added tool ───────────────────────────

describe("refreshRegistry — detects newly added tool", () => {
  it("refresh picks up a tool added to a running service after init", async () => {
    const svc = await startMockService("memory-svc", [
      {
        name: "read_memory",
        description: "Read memory",
        inputSchema: { type: "object", properties: {}, required: [] },
      },
    ]);

    try {
      await writeConfig([{ name: "memory-svc", url: svc.url }]);

      const { initRegistry, refreshRegistry, getServiceForTool } = await import("./registry.js");

      // Init with one tool
      await initRegistry(configPath);
      assert.ok(getServiceForTool("read_memory") !== undefined, "initial tool must be present");
      assert.equal(getServiceForTool("search_memory"), undefined, "new tool must not exist yet");

      // Service adds a new tool
      svc.setTools([
        {
          name: "read_memory",
          description: "Read memory",
          inputSchema: { type: "object", properties: {}, required: [] },
        },
        {
          name: "search_memory",
          description: "Search memory",
          inputSchema: { type: "object", properties: {}, required: [] },
        },
      ]);

      // Trigger one refresh cycle
      await refreshRegistry(configPath);

      // New tool should now be registered
      const searchEntry = getServiceForTool("search_memory");
      assert.ok(searchEntry !== undefined, "newly added tool must be discovered after refresh");
      assert.equal(searchEntry!.serviceName, "memory-svc");
      assert.equal(searchEntry!.serviceUrl, svc.url);

      // Original tool still present
      assert.ok(getServiceForTool("read_memory") !== undefined, "original tool must still be registered");
    } finally {
      await closeService(svc);
    }
  });

  it("refresh removes a tool that was deleted from a service", async () => {
    const svc = await startMockService("memory-svc", [
      {
        name: "read_memory",
        description: "Read memory",
        inputSchema: { type: "object", properties: {}, required: [] },
      },
      {
        name: "old_tool",
        description: "Old tool to remove",
        inputSchema: { type: "object", properties: {}, required: [] },
      },
    ]);

    try {
      await writeConfig([{ name: "memory-svc", url: svc.url }]);

      const { initRegistry, refreshRegistry, getServiceForTool } = await import("./registry.js");

      await initRegistry(configPath);
      assert.ok(getServiceForTool("old_tool") !== undefined, "old_tool must exist before refresh");

      // Service removes old_tool
      svc.setTools([
        {
          name: "read_memory",
          description: "Read memory",
          inputSchema: { type: "object", properties: {}, required: [] },
        },
      ]);

      await refreshRegistry(configPath);

      assert.equal(getServiceForTool("old_tool"), undefined, "removed tool must be gone after refresh");
      assert.ok(getServiceForTool("read_memory") !== undefined, "remaining tool must still be registered");
    } finally {
      await closeService(svc);
    }
  });
});

// ── 4.4: stale service handling ───────────────────────────────────────────────

describe("refreshRegistry — stale service handling", () => {
  it("service goes down: tools retained, service marked stale; service recovers: stale clears", async () => {
    const svc = await startMockService("memory-svc", [
      {
        name: "read_memory",
        description: "Read memory",
        inputSchema: { type: "object", properties: {}, required: [] },
      },
    ]);

    try {
      await writeConfig([{ name: "memory-svc", url: svc.url }]);

      const { initRegistry, refreshRegistry, getServiceForTool, getAllServices, getStaleServices } =
        await import("./registry.js");

      // Init with the service up
      await initRegistry(configPath);
      assert.ok(getServiceForTool("read_memory") !== undefined, "tool must be present after init");
      assert.equal(getStaleServices().length, 0, "no stale services after clean init");

      // Service goes down
      svc.setDown(true);
      await refreshRegistry(configPath);

      // Tools must be retained (from last-known state)
      const entryAfterDown = getServiceForTool("read_memory");
      assert.ok(entryAfterDown !== undefined, "tools must be retained when service goes down");
      assert.equal(entryAfterDown!.serviceName, "memory-svc");

      // Service should now be stale
      const staleAfterDown = getStaleServices();
      assert.ok(staleAfterDown.includes("memory-svc"), "memory-svc must be in stale list after failed refresh");

      // getAllServices stale flag set
      const services = getAllServices();
      const memSvc = services.find((s) => s.serviceName === "memory-svc");
      assert.ok(memSvc !== undefined, "memory-svc must still appear in getAllServices()");
      assert.equal(memSvc!.stale, true, "stale flag must be true on the service entry");

      // Service recovers
      svc.setDown(false);
      await refreshRegistry(configPath);

      // Stale flag must clear
      const staleAfterRecovery = getStaleServices();
      assert.ok(!staleAfterRecovery.includes("memory-svc"), "stale flag must clear after service recovers");

      // Tool still accessible
      assert.ok(getServiceForTool("read_memory") !== undefined, "tool must still be accessible after recovery");

      // getAllServices stale flag cleared
      const servicesAfterRecovery = getAllServices();
      const memSvcRecovered = servicesAfterRecovery.find((s) => s.serviceName === "memory-svc");
      assert.ok(memSvcRecovered !== undefined);
      assert.ok(!memSvcRecovered!.stale, "stale flag must be falsy after recovery");
    } finally {
      await closeService(svc);
    }
  });
});
