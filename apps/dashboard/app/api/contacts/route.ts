import { type NextRequest, NextResponse } from "next/server";

import { daemonFetch } from "@/lib/daemon";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const params = new URLSearchParams();
    const relationship = searchParams.get("relationship");
    const q = searchParams.get("q");
    if (relationship) params.set("relationship", relationship);
    if (q) params.set("q", q);
    const query = params.toString();
    const path = query ? `/api/contacts?${query}` : "/api/contacts";
    const res = await daemonFetch(path);
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: "Daemon unreachable" }, { status: 502 });
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const res = await daemonFetch("/api/contacts", {
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
