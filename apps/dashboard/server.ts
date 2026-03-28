/**
 * Custom server wrapper for the Next.js standalone output.
 *
 * Next.js standalone mode generates its own server.js that calls startServer()
 * from next/dist/server/lib/start-server. That function creates an HTTP server
 * internally. We cannot replace it with a fully custom server because the
 * standalone Next.js bundle strips out webpack and other modules that next()
 * requires during initialization.
 *
 * Instead, this wrapper:
 *  1. Monkey-patches http.createServer to capture the server instance
 *  2. Loads the original standalone server entry (which calls startServer)
 *  3. Attaches a WebSocket "upgrade" handler to proxy /ws/* to the daemon
 */

import http from "http";
import { parse } from "url";
import httpProxy from "http-proxy";

const port = parseInt(process.env.PORT ?? "3000", 10);
const DAEMON_URL = process.env.DAEMON_URL ?? "ws://127.0.0.1:3443";
const DAEMON_WS_URL = DAEMON_URL.replace(/^https:\/\//, "wss://").replace(/^http:\/\//, "ws://");

const proxy = httpProxy.createProxyServer({});
proxy.on("error", (err, _req, res) => {
  console.error("[ws-proxy] error:", err.message);
  if (res && "writeHead" in res) {
    (res as http.ServerResponse).writeHead(502);
    (res as http.ServerResponse).end("Bad Gateway");
  }
});

/** Auth is enabled when BETTER_AUTH_SECRET or DASHBOARD_TOKEN is set. */
function isAuthEnabled(): boolean {
  const secret = process.env.BETTER_AUTH_SECRET;
  const legacyToken = process.env.DASHBOARD_TOKEN;
  return (
    (typeof secret === "string" && secret.length > 0) ||
    (typeof legacyToken === "string" && legacyToken.length > 0)
  );
}

/**
 * Validate a WebSocket upgrade request.
 *
 * Checks the Better Auth session by calling our own /api/auth/get-session
 * endpoint (avoids importing @nova/auth into the custom server entrypoint).
 * Falls back to legacy DASHBOARD_TOKEN in the Authorization header or cookie.
 */
async function validateWsUpgrade(req: http.IncomingMessage): Promise<boolean> {
  if (!isAuthEnabled()) return true;

  const cookie = req.headers.cookie ?? "";
  if (cookie) {
    try {
      const res = await fetch(`http://127.0.0.1:${port}/api/auth/get-session`, {
        headers: { cookie },
      });
      if (res.ok) {
        const body = await res.json();
        if (body && typeof body === "object" && "session" in body) return true;
      }
    } catch {
      // Auth endpoint unreachable — fall through to legacy check
    }
  }

  const legacyToken = process.env.DASHBOARD_TOKEN;
  if (legacyToken) {
    const authHeader = req.headers.authorization;
    if (authHeader?.startsWith("Bearer ") && authHeader.slice(7) === legacyToken) {
      return true;
    }
    const match = cookie
      .split("; ")
      .find((c) => c.startsWith("dashboard_token="));
    if (match) {
      const value = decodeURIComponent(match.split("=")[1] ?? "");
      if (value === legacyToken) return true;
    }
  }

  return false;
}

// ---------------------------------------------------------------------------
// Monkey-patch http.createServer to intercept the server Next.js creates
// ---------------------------------------------------------------------------
const originalCreateServer = http.createServer;

(http as any).createServer = function patchedCreateServer(...args: any[]) {
  const server: http.Server = originalCreateServer.apply(this, args as any);

  server.on("upgrade", async (req, socket, head) => {
    const pathname = parse(req.url ?? "", true).pathname ?? "";

    if (pathname.startsWith("/ws/")) {
      const valid = await validateWsUpgrade(req);
      if (!valid) {
        socket.write("HTTP/1.1 401 Unauthorized\r\n\r\n");
        socket.destroy();
        return;
      }
      proxy.ws(req, socket, head, { target: DAEMON_WS_URL });
      return;
    }

    // Non-/ws/ upgrades (e.g. Next.js HMR) pass through
  });

  // Restore original so subsequent createServer calls aren't patched
  (http as any).createServer = originalCreateServer;

  return server;
};

// ---------------------------------------------------------------------------
// Load the original standalone server entry point
// ---------------------------------------------------------------------------
require("./server.next.js");
