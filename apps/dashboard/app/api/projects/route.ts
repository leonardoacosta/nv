import { NextResponse } from "next/server";

import { DAEMON_URL } from "@/lib/daemon";

export async function GET() {
  try {
    const url = new URL("/api/projects", DAEMON_URL);
    const res = await fetch(url.toString());
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: "Daemon unreachable" }, { status: 502 });
  }
}
