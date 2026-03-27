import { NextResponse, type NextRequest } from "next/server";
import { fleetFetch } from "@/lib/fleet";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const params = new URLSearchParams();

    for (const [key, value] of searchParams.entries()) {
      if (["limit", "offset", "channel", "search"].includes(key)) {
        params.set(key, value);
      }
    }

    const query = params.toString();
    const path = query ? `/recent?${query}` : "/recent";
    const res = await fleetFetch("messages-svc", path);

    if (!res.ok) {
      return NextResponse.json(
        { error: `messages-svc returned ${res.status}` },
        { status: res.status },
      );
    }

    const data = await res.json();

    // Fleet returns { result, error } — unwrap to match MessagesGetResponse
    if (data.error) {
      return NextResponse.json({ error: data.error }, { status: 500 });
    }

    const messages = data.result ?? data.messages ?? data;
    return NextResponse.json({
      messages: Array.isArray(messages) ? messages : [],
      limit: parseInt(searchParams.get("limit") ?? "50", 10),
      offset: parseInt(searchParams.get("offset") ?? "0", 10),
    });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json(
      { error: `Fleet unreachable: ${message}` },
      { status: 502 },
    );
  }
}
