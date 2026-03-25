import { NextRequest, NextResponse } from "next/server";

import { sessionManager } from "@/lib/session-manager";

const DASHBOARD_SECRET = process.env.DASHBOARD_SECRET ?? "";

function validateBearer(req: NextRequest): boolean {
  const auth = req.headers.get("authorization") ?? "";
  if (!auth.startsWith("Bearer ")) return false;
  const token = auth.slice("Bearer ".length).trim();
  return token === DASHBOARD_SECRET && DASHBOARD_SECRET.length > 0;
}

export async function POST(req: NextRequest): Promise<NextResponse> {
  // Auth check
  if (!validateBearer(req)) {
    const status = (req.headers.get("authorization") ?? "").length > 0 ? 403 : 401;
    return NextResponse.json({ error: "Unauthorized" }, { status });
  }

  let body: {
    message_id?: string;
    chat_id?: string;
    text: string;
    context?: Record<string, unknown>;
  };

  try {
    body = (await req.json()) as typeof body;
  } catch {
    return NextResponse.json({ error: "Invalid JSON body" }, { status: 400 });
  }

  if (!body.text || typeof body.text !== "string") {
    return NextResponse.json({ error: "Missing required field: text" }, { status: 400 });
  }

  // AbortController for 120s overall request timeout
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), 120_000);

  try {
    const sendPromise = sessionManager.sendMessage(body.text, body.context);

    // Race the send against the abort signal
    const result = await Promise.race<
      Awaited<ReturnType<typeof sessionManager.sendMessage>> | never
    >([
      sendPromise,
      new Promise<never>((_, reject) => {
        controller.signal.addEventListener("abort", () =>
          reject(new Error("Request timed out")),
        );
      }),
    ]);

    const status = sessionManager.getStatus();

    return NextResponse.json({
      reply: result.reply,
      session_state: status.state,
      processing_ms: result.processing_ms,
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);

    if (message.includes("timed out")) {
      return NextResponse.json({ error: "Request timed out" }, { status: 504 });
    }

    if (message.includes("not ready")) {
      return NextResponse.json(
        { error: "Session not ready", detail: message },
        { status: 503 },
      );
    }

    return NextResponse.json({ error: message }, { status: 500 });
  } finally {
    clearTimeout(timeoutId);
  }
}
