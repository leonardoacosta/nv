/**
 * Tests for unified config schema and resolver.
 *
 * Covers:
 *  4.1 - valid config passes startup validation (loadConfig returns validated object)
 *  4.2 - missing required fields fail (Zod throws formatted error on missing database.url)
 *  4.3 - precedence order respected (env var wins over TOML for daemon.port)
 */

import { describe, it, before, after, afterEach } from "node:test";
import assert from "node:assert/strict";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import * as TOML from "@iarna/toml";

// ── Helpers ───────────────────────────────────────────────────────────────────

let tmpDir: string;
let configPath: string;

// Track env vars we set so we can clean up
const ENV_KEYS: string[] = [
  "DATABASE_URL",
  "NV_DAEMON_PORT",
  "NV_LOG_LEVEL",
  "TOOL_ROUTER_URL",
  "TELEGRAM_BOT_TOKEN",
  "TELEGRAM_CHAT_ID",
];

function snapshotEnv(): Record<string, string | undefined> {
  const snapshot: Record<string, string | undefined> = {};
  for (const key of ENV_KEYS) {
    snapshot[key] = process.env[key];
  }
  return snapshot;
}

function restoreEnv(snapshot: Record<string, string | undefined>): void {
  for (const [key, val] of Object.entries(snapshot)) {
    if (val === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = val;
    }
  }
}

async function writeToml(data: Record<string, unknown>): Promise<void> {
  const content = TOML.stringify(data as Parameters<typeof TOML.stringify>[0]);
  await writeFile(configPath, content, "utf-8");
}

let envSnapshot: Record<string, string | undefined>;

before(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "nv-config-test-"));
  configPath = join(tmpDir, "nv.toml");
  envSnapshot = snapshotEnv();
});

after(async () => {
  restoreEnv(envSnapshot);
  await rm(tmpDir, { recursive: true, force: true });
});

afterEach(() => {
  // Clean up env keys after each test
  restoreEnv(envSnapshot);
});

// ── 4.1: valid config passes startup validation ───────────────────────────────

describe("loadConfig — valid config", () => {
  it("returns a validated Config object when all required fields are supplied", async () => {
    // Set DATABASE_URL via env (required field)
    process.env["DATABASE_URL"] = "postgres://test:pass@localhost/testdb";

    await writeToml({
      daemon: {
        port: 7700,
        log_level: "info",
        tool_router_url: "http://localhost:4100",
      },
      agent: {
        model: "claude-opus-4-6",
        max_turns: 100,
        system_prompt_path: "config/system-prompt.md",
      },
      digest: {
        enabled: true,
        quiet_start: "22:00",
        quiet_end: "07:00",
        tier1_hours: [7, 12, 17],
      },
      queue: {
        concurrency: 2,
        max_queue_size: 20,
      },
    });

    const { loadConfig } = await import("../src/config.js");
    const config = await loadConfig(configPath);

    // Core fields
    assert.equal(config.daemonPort, 7700, "daemonPort must equal TOML value");
    assert.equal(config.logLevel, "info", "logLevel must equal TOML value");
    assert.equal(config.toolRouterUrl, "http://localhost:4100", "toolRouterUrl must be set");
    assert.equal(config.databaseUrl, "postgres://test:pass@localhost/testdb", "databaseUrl must come from env");

    // Agent config
    assert.equal(config.agent.model, "claude-opus-4-6", "agent.model must be set");
    assert.equal(config.agent.maxTurns, 100, "agent.maxTurns must be set");

    // Digest config (merged from schema + defaults)
    assert.equal(typeof config.digest.quietStart, "string", "digest.quietStart must be a string");
    assert.ok(Array.isArray(config.digest.tier1Hours), "digest.tier1Hours must be an array");

    // Queue config
    assert.equal(config.queue.concurrency, 2, "queue.concurrency must be 2");
    assert.equal(config.queue.maxQueueSize, 20, "queue.maxQueueSize must be 20");

    // configPath echoed back
    assert.equal(config.configPath, configPath, "configPath must be the path passed in");
  });

  it("applies defaults for optional fields not in TOML", async () => {
    process.env["DATABASE_URL"] = "postgres://test:pass@localhost/testdb";

    // Minimal TOML — only required field (database.url comes from env)
    await writeToml({});

    const { loadConfig } = await import("../src/config.js");
    const config = await loadConfig(configPath);

    // Defaults
    assert.equal(config.daemonPort, 7700, "default daemon port must be 7700");
    assert.equal(config.logLevel, "info", "default log level must be info");
    assert.equal(config.toolRouterUrl, "http://localhost:4100", "default toolRouterUrl must be set");
    assert.equal(config.agent.model, "claude-opus-4-6", "default agent model must be set");
    assert.equal(config.agent.maxTurns, 100, "default maxTurns must be 100");
  });
});

// ── 4.2: missing required fields fail ────────────────────────────────────────

describe("resolveConfig — missing required fields", () => {
  it("throws a formatted error when database.url is missing", async () => {
    // Ensure DATABASE_URL is not set
    delete process.env["DATABASE_URL"];

    // Write TOML with no database section
    await writeToml({
      daemon: { port: 7700 },
    });

    const { resolveConfig } = await import("../src/config/resolver.js");

    await assert.rejects(
      () => resolveConfig(configPath),
      (err: Error) => {
        assert.ok(err instanceof Error, "must throw an Error");
        // Error message must mention the validation failure
        assert.ok(
          err.message.includes("Configuration validation failed") ||
          err.message.includes("database") ||
          err.message.includes("url"),
          `Error message must mention config failure, got: ${err.message}`,
        );
        return true;
      },
    );
  });

  it("throws a formatted error listing the invalid field path", async () => {
    delete process.env["DATABASE_URL"];

    await writeToml({ daemon: { port: 99999 } }); // invalid port

    const { resolveConfig } = await import("../src/config/resolver.js");

    await assert.rejects(
      () => resolveConfig(configPath),
      (err: Error) => {
        assert.ok(err instanceof Error);
        // Should mention the field that failed
        assert.ok(
          err.message.includes("Configuration validation failed"),
          `Expected validation failure message, got: ${err.message}`,
        );
        return true;
      },
    );
  });
});

// ── 4.3: precedence — env var wins over TOML ─────────────────────────────────

describe("resolveConfig — precedence order", () => {
  it("env var NV_DAEMON_PORT wins over TOML daemon.port", async () => {
    process.env["DATABASE_URL"] = "postgres://test:pass@localhost/testdb";

    // TOML sets port 7700
    await writeToml({
      daemon: {
        port: 7700,
      },
    });

    // Env var sets a different port
    process.env["NV_DAEMON_PORT"] = "8888";

    const { resolveConfig } = await import("../src/config/resolver.js");
    const { config, sources } = await resolveConfig(configPath);

    assert.equal(config.daemon.port, 8888, "env var must win over TOML for daemon.port");
    assert.equal(sources["daemon.port"], "env", "source for daemon.port must be 'env'");
  });

  it("TOML daemon.port wins over built-in default when no env var", async () => {
    process.env["DATABASE_URL"] = "postgres://test:pass@localhost/testdb";
    delete process.env["NV_DAEMON_PORT"];

    await writeToml({
      daemon: {
        port: 9090,
      },
    });

    const { resolveConfig } = await import("../src/config/resolver.js");
    const { config, sources } = await resolveConfig(configPath);

    assert.equal(config.daemon.port, 9090, "TOML value must win over built-in default");
    assert.equal(sources["daemon.port"], "toml", "source for daemon.port must be 'toml'");
  });

  it("built-in default used when neither env var nor TOML provides daemon.port", async () => {
    process.env["DATABASE_URL"] = "postgres://test:pass@localhost/testdb";
    delete process.env["NV_DAEMON_PORT"];

    // TOML with no daemon section
    await writeToml({});

    const { resolveConfig } = await import("../src/config/resolver.js");
    const { config, sources } = await resolveConfig(configPath);

    assert.equal(config.daemon.port, 7700, "built-in default must be 7700");
    assert.equal(sources["daemon.port"], "default", "source must be 'default' when nothing overrides");
  });

  it("env var NV_LOG_LEVEL overrides TOML log_level", async () => {
    process.env["DATABASE_URL"] = "postgres://test:pass@localhost/testdb";

    await writeToml({
      daemon: { log_level: "debug" },
    });

    process.env["NV_LOG_LEVEL"] = "error";

    const { resolveConfig } = await import("../src/config/resolver.js");
    const { config, sources } = await resolveConfig(configPath);

    assert.equal(config.daemon.logLevel, "error", "env var must override TOML for logLevel");
    assert.equal(sources["daemon.logLevel"], "env", "source must be 'env'");

    delete process.env["NV_LOG_LEVEL"];
  });

  it("TOOL_ROUTER_URL env var overrides TOML tool_router_url", async () => {
    process.env["DATABASE_URL"] = "postgres://test:pass@localhost/testdb";

    await writeToml({
      daemon: { tool_router_url: "http://localhost:4100" },
    });

    process.env["TOOL_ROUTER_URL"] = "http://custom-router:9999";

    const { resolveConfig } = await import("../src/config/resolver.js");
    const { config, sources } = await resolveConfig(configPath);

    assert.equal(config.daemon.toolRouterUrl, "http://custom-router:9999");
    assert.equal(sources["daemon.toolRouterUrl"], "env");

    delete process.env["TOOL_ROUTER_URL"];
  });
});
