import { readFileSync } from "node:fs";
import { join } from "node:path";
import { createServer } from "node:http";
import { Hono } from "hono";
import { logger as honoLogger } from "hono/logger";
import { cors } from "hono/cors";
import { secureHeaders } from "hono/secure-headers";
import { serve } from "@hono/node-server";
import { WebSocketServer, type WebSocket } from "ws";
import { Pool } from "pg";
import { logger } from "../logger.js";
import { loadConfig } from "../config.js";
import { handleDiaryGet } from "../http/routes/diary.js";
import { handleBriefingGet, handleBriefingHistory } from "../http/routes/briefing.js";
import { createMemoryService, getMemory, putMemory, type MemoryService } from "../features/memory/index.js";

// ---------------------------------------------------------------------------
// Version — read once at module load time
// ---------------------------------------------------------------------------
function readVersion(): string {
  try {
    const pkgPath = join(import.meta.dirname, "..", "..", "package.json");
    const pkg = JSON.parse(readFileSync(pkgPath, "utf-8")) as { version: string };
    return pkg.version;
  } catch {
    return "unknown";
  }
}

const VERSION = readVersion();

// ---------------------------------------------------------------------------
// Database pool — created lazily when startApiServer is called
// ---------------------------------------------------------------------------
let pool: Pool | null = null;

function getPool(): Pool {
  if (!pool) {
    throw new Error("Database pool not initialised — call startApiServer first");
  }
  return pool;
}

// ---------------------------------------------------------------------------
// Memory service — initialised in startApiServer alongside the pool
// ---------------------------------------------------------------------------
let _memoryService: MemoryService | null = null;

function getMemoryService(): MemoryService {
  if (!_memoryService) {
    throw new Error("Memory service not initialised — call startApiServer first");
  }
  return _memoryService;
}

// ---------------------------------------------------------------------------
// ObligationActivityEvent + ActivityRingBuffer
// ---------------------------------------------------------------------------
export interface ObligationActivityEvent {
  id: string;
  obligationId: string;
  type: "detected" | "started" | "tool_called" | "completed" | "failed";
  payload?: Record<string, unknown>;
  timestamp: string; // ISO-8601
}

export class ActivityRingBuffer {
  private readonly capacity: number;
  private readonly items: ObligationActivityEvent[] = [];

  constructor(capacity = 200) {
    this.capacity = capacity;
  }

  push(event: ObligationActivityEvent): void {
    if (this.items.length >= this.capacity) {
      this.items.shift(); // FIFO eviction
    }
    this.items.push(event);
  }

  recent(n: number): ObligationActivityEvent[] {
    const start = Math.max(0, this.items.length - n);
    return this.items.slice(start);
  }

  all(): ObligationActivityEvent[] {
    return [...this.items];
  }
}

const ringBuffer = new ActivityRingBuffer(200);

// ---------------------------------------------------------------------------
// WebSocket client registry
// ---------------------------------------------------------------------------
const wsClients = new Set<WebSocket>();

// ---------------------------------------------------------------------------
// emitObligationEvent — push to ring buffer and broadcast
// ---------------------------------------------------------------------------
export function emitObligationEvent(event: ObligationActivityEvent): void {
  ringBuffer.push(event);
  const message = JSON.stringify({ type: "event", event });
  for (const client of wsClients) {
    if (client.readyState === 1 /* OPEN */) {
      client.send(message);
    } else {
      wsClients.delete(client);
    }
  }
}

// ---------------------------------------------------------------------------
// Config masking helper
// ---------------------------------------------------------------------------
const SENSITIVE_KEYS = /token|key|secret|password|api_key/i;

function maskConfig(value: unknown, keyName = ""): unknown {
  if (value === null || value === undefined) return value;
  if (typeof value === "string") {
    return SENSITIVE_KEYS.test(keyName) ? "[redacted]" : value;
  }
  if (typeof value === "object" && !Array.isArray(value)) {
    const obj = value as Record<string, unknown>;
    const masked: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(obj)) {
      masked[k] = maskConfig(v, k);
    }
    return masked;
  }
  return value;
}

// ---------------------------------------------------------------------------
// Tool registry placeholder — replaced by future spec
// ---------------------------------------------------------------------------
export async function executeToolByName(
  toolName: string,
  _input: Record<string, unknown>,
): Promise<string> {
  throw new Error(`Unknown tool: ${toolName}. Tool registry not yet initialised.`);
}

// ---------------------------------------------------------------------------
// Hono app
// ---------------------------------------------------------------------------
const app = new Hono();

// Middleware
app.use("*", honoLogger());
app.use(
  "*",
  cors({
    origin: "https://nova.leonardoacosta.dev",
    credentials: true,
  }),
);
app.use("*", secureHeaders());

// Global error handler
app.onError((err, c) => {
  logger.error({ err }, "Unhandled API error");
  return c.json({ error: err instanceof Error ? err.message : String(err), status: 500 }, 500);
});

// ---------------------------------------------------------------------------
// GET /health
// ---------------------------------------------------------------------------
app.get("/health", (c) => {
  return c.json({
    status: "ok",
    uptime_secs: Math.floor(process.uptime()),
    version: VERSION,
  });
});

// ---------------------------------------------------------------------------
// GET /api/config
// ---------------------------------------------------------------------------
app.get("/api/config", async (c) => {
  const config = await loadConfig();
  return c.json(maskConfig(config));
});

// ---------------------------------------------------------------------------
// GET /api/obligations
// ---------------------------------------------------------------------------
app.get("/api/obligations", async (c) => {
  const status = c.req.query("status");
  const owner = c.req.query("owner");

  const conditions: string[] = [];
  const params: unknown[] = [];
  let idx = 1;

  if (status) {
    conditions.push(`status = $${idx++}`);
    params.push(status);
  }
  if (owner) {
    conditions.push(`owner = $${idx++}`);
    params.push(owner);
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : "";
  const sql = `SELECT * FROM obligations ${where} ORDER BY created_at DESC`;

  const result = await getPool().query(sql, params);
  return c.json(result.rows);
});

// ---------------------------------------------------------------------------
// GET /api/obligations/stats
// ---------------------------------------------------------------------------
app.get("/api/obligations/stats", async (c) => {
  const db = getPool();

  const [totalRes, byStatusRes, byOwnerRes] = await Promise.all([
    db.query("SELECT COUNT(*)::int AS total FROM obligations"),
    db.query("SELECT status, COUNT(*)::int AS count FROM obligations GROUP BY status"),
    db.query("SELECT owner, COUNT(*)::int AS count FROM obligations GROUP BY owner"),
  ]);

  const by_status: Record<string, number> = {};
  for (const row of byStatusRes.rows as Array<{ status: string; count: number }>) {
    by_status[row.status] = row.count;
  }

  const by_owner: Record<string, number> = {};
  for (const row of byOwnerRes.rows as Array<{ owner: string; count: number }>) {
    by_owner[row.owner] = row.count;
  }

  return c.json({
    total: (totalRes.rows[0] as { total: number }).total,
    by_status,
    by_owner,
  });
});

// ---------------------------------------------------------------------------
// GET /api/messages
// ---------------------------------------------------------------------------
app.get("/api/messages", async (c) => {
  const rawPage = parseInt(c.req.query("page") ?? "1", 10);
  const rawPerPage = parseInt(c.req.query("per_page") ?? "50", 10);
  const channel = c.req.query("channel");

  const page = Math.max(1, isNaN(rawPage) ? 1 : rawPage);
  const perPage = Math.min(200, Math.max(1, isNaN(rawPerPage) ? 50 : rawPerPage));
  const offset = (page - 1) * perPage;

  const conditions: string[] = [];
  const params: unknown[] = [];
  let idx = 1;

  if (channel) {
    conditions.push(`channel = $${idx++}`);
    params.push(channel);
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : "";

  const [dataRes, countRes] = await Promise.all([
    getPool().query(
      `SELECT * FROM messages ${where} ORDER BY created_at DESC LIMIT $${idx++} OFFSET $${idx}`,
      [...params, perPage, offset],
    ),
    getPool().query(`SELECT COUNT(*)::int AS total FROM messages ${where}`, params),
  ]);

  return c.json({
    messages: dataRes.rows,
    total: (countRes.rows[0] as { total: number }).total,
    page,
    per_page: perPage,
  });
});

// ---------------------------------------------------------------------------
// GET /api/diary
// ---------------------------------------------------------------------------
app.get("/api/diary", handleDiaryGet);

// ---------------------------------------------------------------------------
// GET /api/briefing
// ---------------------------------------------------------------------------
app.get("/api/briefing", (c) => handleBriefingGet(c, getPool));

// ---------------------------------------------------------------------------
// GET /api/briefing/history
// ---------------------------------------------------------------------------
app.get("/api/briefing/history", (c) => handleBriefingHistory(c, getPool));

// ---------------------------------------------------------------------------
// GET /api/memory
// ---------------------------------------------------------------------------
app.get("/api/memory", (c) => getMemory(c, getMemoryService));

// ---------------------------------------------------------------------------
// PUT /api/memory
// ---------------------------------------------------------------------------
app.put("/api/memory", (c) => putMemory(c, getMemoryService));

// ---------------------------------------------------------------------------
// POST /api/tool-call
// ---------------------------------------------------------------------------
app.post("/api/tool-call", async (c) => {
  // Local-only guard
  const forwarded = c.req.header("x-forwarded-for");
  if (forwarded) {
    return c.json({ error: "Forbidden — local only" }, 403);
  }

  // Check peer IP via incoming bindings (only available when served via @hono/node-server)
  const env = c.env as { incoming?: { socket?: { remoteAddress?: string } } } | undefined;
  const peerIp = env?.incoming?.socket?.remoteAddress ?? "";
  const isLocal = peerIp === "127.0.0.1" || peerIp === "::1" || peerIp === "::ffff:127.0.0.1";
  if (!isLocal) {
    return c.json({ error: "Forbidden — local only" }, 403);
  }

  const body = await c.req.json<{ tool_name?: string; input?: Record<string, unknown> }>();
  const { tool_name, input } = body;

  if (!tool_name || typeof tool_name !== "string") {
    return c.json({ result: null, error: "tool_name is required" });
  }

  try {
    const result = await executeToolByName(tool_name, input ?? {});
    return c.json({ result, error: null });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return c.json({ result: null, error: message });
  }
});

// ---------------------------------------------------------------------------
// startApiServer — creates HTTP server, attaches WebSocket, starts listening
// ---------------------------------------------------------------------------
export async function startApiServer(port: number): Promise<void> {
  // Initialise pool and memory service with config
  const config = await loadConfig();
  pool = new Pool({ connectionString: config.databaseUrl });
  _memoryService = createMemoryService(pool);

  // Create Node.js HTTP server wrapping Hono
  const server = createServer();

  // Wire Hono as the request handler
  const nodeServer = serve(
    {
      fetch: app.fetch,
      port,
      // We manage the server ourselves for WebSocket support
      createServer: () => server,
    },
    (info) => {
      logger.info({ port: info.port }, `API server listening on :${info.port}`);
    },
  );

  // Attach WebSocket server for /ws/events
  const wss = new WebSocketServer({ noServer: true });

  nodeServer.on("upgrade", (request, socket, head) => {
    const url = request.url ?? "";
    if (url === "/ws/events" || url.startsWith("/ws/events?")) {
      wss.handleUpgrade(request, socket, head, (ws) => {
        wss.emit("connection", ws, request);
      });
    } else {
      socket.destroy();
    }
  });

  wss.on("connection", (ws) => {
    wsClients.add(ws);

    // Send snapshot of recent 50 events
    const snapshot = JSON.stringify({
      type: "snapshot",
      events: ringBuffer.recent(50),
    });
    ws.send(snapshot);

    ws.on("close", () => {
      wsClients.delete(ws);
    });

    ws.on("error", () => {
      wsClients.delete(ws);
    });
  });
}

export { app };
