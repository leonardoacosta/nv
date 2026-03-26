import { type NextRequest, NextResponse } from "next/server";

import { daemonFetch } from "@/lib/daemon";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const limit = searchParams.get("limit");
    const path = limit
      ? `/api/cold-starts?limit=${encodeURIComponent(limit)}`
      : "/api/cold-starts";
    const res = await daemonFetch(path);
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: "Daemon unreachable" }, { status: 502 });
  }
}
