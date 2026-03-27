import { NextResponse } from "next/server";
import { fleetFetch } from "@/lib/fleet";
import type { StatsGetResponse, ToolStatsReport } from "@/types/api";

export async function GET() {
  try {
    const res = await fleetFetch("meta-svc", "/services");

    if (!res.ok) {
      // Return empty stats rather than an error for degraded state
      const empty: StatsGetResponse = {
        tool_usage: {
          total_invocations: 0,
          invocations_today: 0,
          per_tool: [],
        },
      };
      return NextResponse.json(empty);
    }

    const data = await res.json();

    // meta-svc /services returns tool usage aggregates
    const toolUsage: ToolStatsReport = {
      total_invocations: data.total_invocations ?? 0,
      invocations_today: data.invocations_today ?? 0,
      per_tool: data.per_tool ?? data.tools ?? [],
    };

    const response: StatsGetResponse = {
      tool_usage: toolUsage,
      ...data,
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
