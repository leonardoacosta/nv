import { NextResponse } from "next/server";

const DAEMON_URL = process.env["DAEMON_URL"] ?? "http://localhost:7700";
const CONNECT_TIMEOUT_MS = 10_000;

export async function GET() {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), CONNECT_TIMEOUT_MS);

  let daemonResponse: Response;
  try {
    daemonResponse = await fetch(`${DAEMON_URL}/api/briefing/stream`, {
      signal: controller.signal,
    });
  } catch (err: unknown) {
    clearTimeout(timeoutId);
    const isAbort = err instanceof Error && err.name === "AbortError";
    const reason = isAbort ? "connect timeout" : "connection failed";
    console.error(`Daemon unavailable (${reason}):`, err);
    const errorEvent =
      `data: ${JSON.stringify({ type: "error", message: "daemon_unavailable" })}\n\n`;
    return new Response(errorEvent, {
      status: 503,
      headers: {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
        Connection: "keep-alive",
      },
    });
  }

  clearTimeout(timeoutId);

  if (!daemonResponse.ok || !daemonResponse.body) {
    const errorEvent =
      `data: ${JSON.stringify({ type: "error", message: "daemon_unavailable" })}\n\n`;
    return new Response(errorEvent, {
      status: 503,
      headers: {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
        Connection: "keep-alive",
      },
    });
  }

  return new Response(daemonResponse.body, {
    status: 200,
    headers: {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      Connection: "keep-alive",
    },
  });
}

// Required for Next.js App Router streaming responses
export const dynamic = "force-dynamic";
