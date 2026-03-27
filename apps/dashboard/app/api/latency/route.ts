import { NextResponse } from "next/server";
import { fleetFetch } from "@/lib/fleet";

export async function GET() {
  try {
    const res = await fleetFetch("meta-svc", "/health");

    if (!res.ok) {
      return NextResponse.json(
        { error: `meta-svc returned ${res.status}` },
        { status: res.status },
      );
    }

    const data = await res.json();

    // Extract per-service latency from the health response
    const services = data.services ?? {};
    const latency: Record<string, number | null> = {};

    for (const [name, info] of Object.entries(services)) {
      const svcInfo = info as { latency_ms?: number; latency?: number };
      latency[name] = svcInfo.latency_ms ?? svcInfo.latency ?? null;
    }

    return NextResponse.json({
      services: latency,
      timestamp: new Date().toISOString(),
    });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json(
      { error: `Fleet unreachable: ${message}` },
      { status: 502 },
    );
  }
}
