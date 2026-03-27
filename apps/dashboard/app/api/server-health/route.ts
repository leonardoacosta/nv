import { NextResponse } from "next/server";
import { fleetFetch } from "@/lib/fleet";
import type { ServerHealthGetResponse } from "@/types/api";

export async function GET() {
  try {
    const res = await fleetFetch("tool-router", "/health");

    if (!res.ok) {
      const response: ServerHealthGetResponse = {
        daemon: {},
        latest: null,
        status: "critical",
        history: [],
      };
      return NextResponse.json(response, { status: 200 });
    }

    const data = await res.json();

    // tool-router /health returns a services map with status/latency per service
    const response: ServerHealthGetResponse = {
      daemon: data.services ?? data,
      latest: null,
      status: data.status ?? "healthy",
      history: [],
    };

    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json(
      { error: `Fleet unreachable: ${message}` },
      { status: 502 },
    );
  }
}
