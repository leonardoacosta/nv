import { type NextRequest, NextResponse } from "next/server";

import { DAEMON_URL } from "@/lib/daemon";

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const url = new URL("/api/solve", DAEMON_URL);
    const res = await fetch(url.toString(), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: "Daemon unreachable" }, { status: 502 });
  }
}
