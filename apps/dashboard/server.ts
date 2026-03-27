import { createServer, type IncomingMessage } from "http";
import { parse } from "url";
import next from "next";
import { auth } from "@nova/auth";

const dev = process.env.NODE_ENV !== "production";
const port = parseInt(process.env.PORT ?? "3000", 10);

const app = next({ dev });
const handle = app.getRequestHandler();

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
 * Validate a WebSocket upgrade request using Better Auth session or legacy
 * API key in the Authorization header.
 */
async function validateWsUpgrade(req: IncomingMessage): Promise<boolean> {
  if (!isAuthEnabled()) return true;

  // Try Better Auth session from cookies on the upgrade request
  const headers = new Headers();
  for (const [key, value] of Object.entries(req.headers)) {
    if (typeof value === "string") {
      headers.set(key, value);
    } else if (Array.isArray(value)) {
      headers.set(key, value.join(", "));
    }
  }

  const session = await auth.api.getSession({ headers });
  if (session) return true;

  // Fallback: legacy DASHBOARD_TOKEN in Authorization header or cookie
  const legacyToken = process.env.DASHBOARD_TOKEN;
  if (legacyToken) {
    const authHeader = req.headers.authorization;
    if (authHeader?.startsWith("Bearer ") && authHeader.slice(7) === legacyToken) {
      return true;
    }
    // Check legacy cookie
    const cookieHeader = req.headers.cookie ?? "";
    const match = cookieHeader
      .split("; ")
      .find((c) => c.startsWith("dashboard_token="));
    if (match) {
      const value = decodeURIComponent(match.split("=")[1] ?? "");
      if (value === legacyToken) return true;
    }
  }

  return false;
}

app.prepare().then(() => {
  const server = createServer((req, res) => {
    const parsedUrl = parse(req.url!, true);
    handle(req, res, parsedUrl);
  });

  // Validate session on WebSocket upgrade requests before they reach the proxy
  server.on("upgrade", async (req, socket, _head) => {
    const pathname = parse(req.url ?? "", true).pathname ?? "";

    if (pathname.startsWith("/ws/")) {
      const valid = await validateWsUpgrade(req);
      if (!valid) {
        socket.write("HTTP/1.1 401 Unauthorized\r\n\r\n");
        socket.destroy();
        return;
      }
    }

    // Non-/ws/ upgrades (e.g. Next.js HMR) and authenticated /ws/ upgrades
    // pass through to the default upgrade handler
  });

  server.listen(port, () => {
    console.log(`server listening on port ${port}`);
  });
});
