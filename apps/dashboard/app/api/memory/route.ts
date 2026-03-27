import { type NextRequest, NextResponse } from "next/server";
import { fleetFetch } from "@/lib/fleet";

export async function GET(request: NextRequest) {
  try {
    const topic = request.nextUrl.searchParams.get("topic");

    if (topic) {
      // Read a specific topic
      const res = await fleetFetch("memory-svc", "/read", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ topic }),
      });

      if (!res.ok) {
        return NextResponse.json(
          { error: `memory-svc returned ${res.status}` },
          { status: res.status },
        );
      }

      const data = await res.json();
      return NextResponse.json({
        topic,
        content: data.content ?? data.result ?? "",
      });
    }

    // No topic — return list of topics
    const res = await fleetFetch("memory-svc", "/read", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({}),
    });

    if (!res.ok) {
      return NextResponse.json(
        { error: `memory-svc returned ${res.status}` },
        { status: res.status },
      );
    }

    const data = await res.json();
    return NextResponse.json({
      topics: data.topics ?? data.result ?? [],
    });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json(
      { error: `Fleet unreachable: ${message}` },
      { status: 502 },
    );
  }
}

export async function PUT(request: NextRequest) {
  try {
    const body = await request.json();

    const res = await fleetFetch("memory-svc", "/write", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        topic: body.topic,
        content: body.content,
      }),
    });

    if (!res.ok) {
      return NextResponse.json(
        { error: `memory-svc returned ${res.status}` },
        { status: res.status },
      );
    }

    const data = await res.json();
    return NextResponse.json({
      topic: body.topic,
      written: data.written ?? (body.content as string).length,
    });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json(
      { error: `Fleet unreachable: ${message}` },
      { status: 502 },
    );
  }
}
