import { createServer } from "http";
import { parse } from "url";
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

app.prepare().then(() => {
  const server = createServer((req, res) => {
    const parsedUrl = parse(req.url!, true);
    handle(req, res, parsedUrl);
  });

  server.on("upgrade", (req, socket, head) => {
    if (req.url === "/ws/events") {
      proxy.ws(req, socket, head, {
        target: DAEMON_WS_URL,
      });
    } else {
      socket.destroy();
    }
  });

  server.listen(port, () => {
    console.log(`server listening on port ${port}`);
  });
});
