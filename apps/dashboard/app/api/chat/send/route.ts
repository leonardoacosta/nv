import { NextResponse } from "next/server";

const DAEMON_URL = process.env["DAEMON_URL"] ?? "http://localhost:7700";
const CONNECT_TIMEOUT_MS = 10_000;

export async function POST(request: Request) {
  try {
    const body = (await request.json()) as { message?: string };

    if (!body.message || typeof body.message !== "string") {
      return NextResponse.json(
        { error: "Missing required field: message" },
        { status: 400 },
      );
    }

    // Create an AbortController for connect timeout
    const controller = new AbortController();
    const timeoutId = setTimeout(
      () => controller.abort(),
      CONNECT_TIMEOUT_MS,
    );

    let daemonResponse: Response;
    try {
      daemonResponse = await fetch(`${DAEMON_URL}/chat`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: body.message }),
        signal: controller.signal,
      });
    } catch (err: unknown) {
      clearTimeout(timeoutId);
      // Connection refused, timeout, or network error
      const isAbort =
        err instanceof Error && err.name === "AbortError";
      const reason = isAbort ? "connect timeout" : "connection failed";
      console.error(`Daemon unavailable (${reason}):`, err);
      return NextResponse.json(
        { error: "daemon_unavailable", fallback: "telegram" },
        { status: 503 },
      );
    }

    clearTimeout(timeoutId);

    if (daemonResponse.status === 503 || !daemonResponse.ok) {
      return NextResponse.json(
        { error: "daemon_unavailable", fallback: "telegram" },
        { status: 503 },
      );
    }

    // Stream the SSE response back to the client
    if (!daemonResponse.body) {
      return NextResponse.json(
        { error: "daemon_unavailable", fallback: "telegram" },
        { status: 503 },
      );
    }

    return new Response(daemonResponse.body, {
      status: 200,
      headers: {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
        Connection: "keep-alive",
      },
    });
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : "Unknown error";
    console.error("Chat send error:", message);
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
