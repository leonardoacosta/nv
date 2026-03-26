import { createServer } from "http";
import { parse } from "url";
import { timingSafeEqual } from "crypto";
import next from "next";
import httpProxy from "http-proxy";

const dev = process.env.NODE_ENV !== "production";
const port = parseInt(process.env.PORT ?? "3000", 10);

const DAEMON_URL = process.env.DAEMON_URL ?? "ws://127.0.0.1:3443";
const DAEMON_WS_URL = DAEMON_URL.replace(/^https:\/\//, "wss://").replace(
  /^http:\/\//,
  "ws://",
);

const app = next({ dev });
const handle = app.getRequestHandler();
const proxy = httpProxy.createProxyServer({});

proxy.on("error", (err, _req, res) => {
  console.error("[ws-proxy] error:", err.message);
  if (res && "writeHead" in res) {
    (res as import("http").ServerResponse).writeHead(502);
    (res as import("http").ServerResponse).end("Bad Gateway");
  }
});

function verifyWsToken(candidate: string): boolean {
  const token = process.env.DASHBOARD_TOKEN;
  if (!token) return false;
  const a = Buffer.from(candidate);
  const b = Buffer.from(token);
  if (a.length !== b.length) {
    timingSafeEqual(b, b);
    return false;
  }
  return timingSafeEqual(a, b);
}

app.prepare().then(() => {
  const server = createServer((req, res) => {
    const parsedUrl = parse(req.url!, true);
    handle(req, res, parsedUrl);
  });

  server.on("upgrade", (req, socket, head) => {
    const parsed = parse(req.url ?? "", true);
    const isWsEvents =
      parsed.pathname === "/ws/events" ||
      (req.url ?? "").startsWith("/ws/events");

    if (!isWsEvents) {
      socket.destroy();
      return;
    }

    // Validate token if DASHBOARD_TOKEN is set (skip in dev mode)
    const dashboardToken = process.env.DASHBOARD_TOKEN;
    if (dashboardToken && dashboardToken.length > 0) {
      const token = parsed.query?.token;
      const candidate = Array.isArray(token) ? token[0] : token;
      if (!candidate || !verifyWsToken(candidate)) {
        socket.write("HTTP/1.1 401 Unauthorized\r\n\r\n");
        socket.destroy();
        return;
      }
    }

    proxy.ws(req, socket, head, {
      target: DAEMON_WS_URL,
    });
  });

  server.listen(port, () => {
    console.log(`server listening on port ${port}`);
  });
});
