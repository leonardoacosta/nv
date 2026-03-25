import { type NextRequest, NextResponse } from "next/server";

import { DAEMON_URL } from "@/lib/daemon";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const url = new URL("/api/obligations", DAEMON_URL);
    const status = searchParams.get("status");
    const owner = searchParams.get("owner");
    if (status) url.searchParams.set("status", status);
    if (owner) url.searchParams.set("owner", owner);
    const res = await fetch(url.toString());
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: "Daemon unreachable" }, { status: 502 });
  }
}
