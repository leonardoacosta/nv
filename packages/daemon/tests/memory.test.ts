import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { mkdtemp } from "node:fs/promises";

// ─── Helpers ──────────────────────────────────────────────────────────────────

function makeMockPool(rows: unknown[] = []) {
  return {
    query: mock.fn(async () => ({ rows })),
  };
}

// ─── MemoryStore ──────────────────────────────────────────────────────────────

describe("MemoryStore", () => {
  it("upsert returns the inserted row", async () => {
    const { MemoryStore } = await import("../src/features/memory/store.js");

    const now = new Date();
    const returnedRow = {
      id: "uuid-1",
      topic: "test-topic",
      content: "hello world",
      updated_at: now,
    };
    const pool = makeMockPool([returnedRow]);
    const store = new MemoryStore(pool as never);

    const record = await store.upsert("test-topic", "hello world");

    assert.equal(record.topic, "test-topic");
    assert.equal(record.content, "hello world");
    assert.ok(pool.query.mock.calls.length >= 1);
  });

  it("get returns null when pool returns empty rows", async () => {
    const { MemoryStore } = await import("../src/features/memory/store.js");

    const pool = makeMockPool([]);
    const store = new MemoryStore(pool as never);

    const result = await store.get("nonexistent");
    assert.equal(result, null);
  });

  it("list returns topic and updated_at for all rows", async () => {
    const { MemoryStore } = await import("../src/features/memory/store.js");

    const now = new Date();
    const rows = [
      { topic: "alpha", updated_at: now },
      { topic: "beta", updated_at: now },
    ];
    const pool = makeMockPool(rows);
    const store = new MemoryStore(pool as never);

    const list = await store.list();
    assert.equal(list.length, 2);
    assert.equal(list[0]?.topic, "alpha");
    assert.equal(list[1]?.topic, "beta");
  });

  it("delete calls pool.query with DELETE statement", async () => {
    const { MemoryStore } = await import("../src/features/memory/store.js");

    const pool = makeMockPool([]);
    const store = new MemoryStore(pool as never);

    await store.delete("old-topic");
    assert.ok(pool.query.mock.calls.length >= 1);
    const call = pool.query.mock.calls[0];
    assert.ok((call?.arguments[0] as string).includes("DELETE"));
  });
});

// ─── sanitizeTopic ────────────────────────────────────────────────────────────

describe("sanitizeTopic", () => {
  it("lowercases the input", async () => {
    const { sanitizeTopic } = await import("../src/features/memory/fs-sync.js");
    assert.equal(sanitizeTopic("Hello World"), "helloworld");
  });

  it("strips spaces and special characters", async () => {
    const { sanitizeTopic } = await import("../src/features/memory/fs-sync.js");
    assert.equal(sanitizeTopic("foo/bar/../baz"), "foobarbaz");
  });

  it("preserves hyphens and underscores", async () => {
    const { sanitizeTopic } = await import("../src/features/memory/fs-sync.js");
    assert.equal(sanitizeTopic("foo-bar_baz"), "foo-bar_baz");
  });

  it("strips dots and slashes", async () => {
    const { sanitizeTopic } = await import("../src/features/memory/fs-sync.js");
    assert.equal(sanitizeTopic("../../etc/passwd"), "etcpasswd");
  });
});

// ─── MemoryFsSync ─────────────────────────────────────────────────────────────

describe("MemoryFsSync", () => {
  it("write then read returns the same content", async () => {
    const { MemoryFsSync } = await import("../src/features/memory/fs-sync.js");

    const dir = await mkdtemp(join(tmpdir(), "nv-memory-test-"));
    const fsSync = new MemoryFsSync(dir);

    await fsSync.write("test-topic", "hello world");
    const content = await fsSync.read("test-topic");
    assert.equal(content, "hello world");
  });

  it("read returns null for missing file", async () => {
    const { MemoryFsSync } = await import("../src/features/memory/fs-sync.js");

    const dir = await mkdtemp(join(tmpdir(), "nv-memory-test-"));
    const fsSync = new MemoryFsSync(dir);

    const result = await fsSync.read("nonexistent");
    assert.equal(result, null);
  });

  it("listTopics returns stem names of .md files", async () => {
    const { MemoryFsSync } = await import("../src/features/memory/fs-sync.js");

    const dir = await mkdtemp(join(tmpdir(), "nv-memory-test-"));
    const fsSync = new MemoryFsSync(dir);

    await fsSync.write("alpha", "a");
    await fsSync.write("beta", "b");
    const topics = await fsSync.listTopics();
    assert.ok(topics.includes("alpha"), `expected alpha in ${JSON.stringify(topics)}`);
    assert.ok(topics.includes("beta"), `expected beta in ${JSON.stringify(topics)}`);
  });

  it("listTopics returns empty array for missing directory", async () => {
    const { MemoryFsSync } = await import("../src/features/memory/fs-sync.js");

    const fsSync = new MemoryFsSync("/tmp/nv-nonexistent-dir-" + Date.now().toString());
    const topics = await fsSync.listTopics();
    assert.deepEqual(topics, []);
  });
});

// ─── MemorySearch ─────────────────────────────────────────────────────────────

describe("MemorySearch.byKeyword", () => {
  it("calls SQL ILIKE query and returns results", async () => {
    const { MemorySearch } = await import("../src/features/memory/search.js");

    const rows = [{ topic: "test", content: "hello world" }];
    const pool = makeMockPool(rows);
    const search = new MemorySearch(pool as never);

    const results = await search.byKeyword("hello");
    assert.equal(results.length, 1);
    assert.equal(results[0]?.topic, "test");
    assert.equal(results[0]?.content, "hello world");
  });

  it("returns empty array when no matches", async () => {
    const { MemorySearch } = await import("../src/features/memory/search.js");

    const pool = makeMockPool([]);
    const search = new MemorySearch(pool as never);

    const results = await search.byKeyword("nothing");
    assert.deepEqual(results, []);
  });
});

describe("MemorySearch.bySimilarity", () => {
  it("throws when no embeddings are configured", async () => {
    const { MemorySearch } = await import("../src/features/memory/search.js");

    // First query (EXISTS check) returns has_embeddings: false
    const pool = {
      query: mock.fn(async () => ({ rows: [{ has_embeddings: false }] })),
    };
    const search = new MemorySearch(pool as never);

    await assert.rejects(
      () => search.bySimilarity([0.1, 0.2, 0.3]),
      /embeddings not configured/,
    );
  });
});

// ─── MemoryService ────────────────────────────────────────────────────────────

describe("MemoryService.upsert", () => {
  it("calls store.upsert and fsSync.write; returns the record", async () => {
    const { MemoryService } = await import("../src/features/memory/service.js");
    const { MemoryStore } = await import("../src/features/memory/store.js");
    const { MemoryFsSync } = await import("../src/features/memory/fs-sync.js");
    const { MemorySearch } = await import("../src/features/memory/search.js");

    const now = new Date();
    const record = { id: "uuid-1", topic: "test", content: "hello", updated_at: now };

    // Mock store
    const mockPool = makeMockPool([record]);
    const store = new MemoryStore(mockPool as never);

    // Mock fsSync using a temp dir
    const dir = await mkdtemp(join(tmpdir(), "nv-service-test-"));
    const fsSync = new MemoryFsSync(dir);

    const searcher = new MemorySearch(mockPool as never);
    const service = new MemoryService(store, fsSync, searcher);

    const result = await service.upsert("test", "hello");
    assert.equal(result.topic, "test");
    assert.equal(result.content, "hello");

    // Verify fs file was written
    const fileContent = await fsSync.read("test");
    assert.equal(fileContent, "hello");
  });

  it("does not throw when fsSync.write fails", async () => {
    const { MemoryService } = await import("../src/features/memory/service.js");
    const { MemoryStore } = await import("../src/features/memory/store.js");
    const { MemorySearch } = await import("../src/features/memory/search.js");

    const now = new Date();
    const record = { id: "uuid-1", topic: "test", content: "hello", updated_at: now };
    const mockPool = makeMockPool([record]);
    const store = new MemoryStore(mockPool as never);

    // FsSync that always throws
    const brokenFsSync = {
      write: async () => { throw new Error("disk full"); },
    };

    const searcher = new MemorySearch(mockPool as never);
    const service = new MemoryService(store, brokenFsSync as never, searcher);

    // Should not throw — fs failure is best-effort
    const result = await service.upsert("test", "hello");
    assert.equal(result.topic, "test");
  });
});
