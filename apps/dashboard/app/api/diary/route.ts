import { type NextRequest, NextResponse } from "next/server";

import { daemonFetch } from "@/lib/daemon";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const params = new URLSearchParams();
    const date = searchParams.get("date");
    const limit = searchParams.get("limit");
    if (date) params.set("date", date);
    if (limit) params.set("limit", limit);
    const query = params.toString();
    const path = query ? `/api/diary?${query}` : "/api/diary";
    const res = await daemonFetch(path);
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: "Daemon unreachable" }, { status: 502 });
  }
}
