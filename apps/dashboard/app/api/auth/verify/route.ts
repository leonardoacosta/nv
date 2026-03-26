import { NextResponse, type NextRequest } from "next/server";
import { verifyToken, isAuthEnabled } from "@/lib/auth";

export async function POST(request: NextRequest) {
  // In dev mode (no DASHBOARD_TOKEN), always succeed
  if (!isAuthEnabled()) {
    return NextResponse.json({ ok: true });
  }

  let body: { token?: string };
  try {
    body = (await request.json()) as { token?: string };
  } catch {
    return NextResponse.json({ error: "Token required" }, { status: 400 });
  }

  const { token } = body;
  if (!token || typeof token !== "string") {
    return NextResponse.json({ error: "Token required" }, { status: 400 });
  }

  if (!verifyToken(token)) {
    return NextResponse.json({ error: "Invalid token" }, { status: 401 });
  }

  return NextResponse.json({ ok: true });
}
