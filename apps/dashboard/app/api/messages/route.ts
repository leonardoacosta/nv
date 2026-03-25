import { NextResponse, type NextRequest } from "next/server";

import { DAEMON_URL } from "@/lib/daemon";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const url = new URL("/api/messages", DAEMON_URL);

    // Forward supported query params
    for (const [key, value] of searchParams.entries()) {
      if (["limit", "offset", "channel", "search"].includes(key)) {
        url.searchParams.set(key, value);
      }
    }

    const res = await fetch(url.toString());
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: "Daemon unreachable" }, { status: 502 });
  }
}
