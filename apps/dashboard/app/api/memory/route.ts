import { type NextRequest, NextResponse } from "next/server";

import { DAEMON_URL } from "@/lib/daemon";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const url = new URL("/api/memory", DAEMON_URL);
    const topic = searchParams.get("topic");
    if (topic) url.searchParams.set("topic", topic);
    const res = await fetch(url.toString());
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: "Daemon unreachable" }, { status: 502 });
  }
}

export async function PUT(request: NextRequest) {
  try {
    const body = await request.json();
    const url = new URL("/api/memory", DAEMON_URL);
    const res = await fetch(url.toString(), {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: "Daemon unreachable" }, { status: 502 });
  }
}
