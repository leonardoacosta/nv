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
  if (!validateBearer(req)) {
    const status = (req.headers.get("authorization") ?? "").length > 0 ? 403 : 401;
    return NextResponse.json({ error: "Unauthorized" }, { status });
  }

  let body: { action: "start" | "stop" | "restart" };

  try {
    body = (await req.json()) as typeof body;
  } catch {
    return NextResponse.json({ error: "Invalid JSON body" }, { status: 400 });
  }

  const { action } = body;

  if (action !== "start" && action !== "stop" && action !== "restart") {
    return NextResponse.json(
      { error: "Invalid action. Must be one of: start, stop, restart" },
      { status: 400 },
    );
  }

  try {
    if (action === "start") {
      await sessionManager.start();
    } else if (action === "stop") {
      await sessionManager.stop();
    } else {
      await sessionManager.restart();
    }

    return NextResponse.json({ status: sessionManager.getStatus() });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
