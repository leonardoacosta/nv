import { type NextRequest, NextResponse } from "next/server";

import { DAEMON_URL } from "@/lib/daemon";

/**
 * POST /api/obligations/[id]/execute
 *
 * Attempts to POST to daemon /api/obligations/{id}/execute.
 * If the daemon returns 404 (endpoint not yet implemented), falls back to
 * a PATCH status update to "in_progress" so the UI still responds correctly.
 */
export async function POST(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;

    // Try the execute endpoint first
    const executeUrl = new URL(`/api/obligations/${id}/execute`, DAEMON_URL);
    const executeRes = await fetch(executeUrl.toString(), { method: "POST" });

    if (executeRes.ok) {
      const data = await executeRes.json();
      return NextResponse.json(data, { status: executeRes.status });
    }

    // Fallback: patch status to in_progress
    const patchUrl = new URL(`/api/obligations/${id}`, DAEMON_URL);
    const patchRes = await fetch(patchUrl.toString(), {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ status: "in_progress" }),
    });
    const data = await patchRes.json();
    return NextResponse.json(data, { status: patchRes.status });
  } catch {
    return NextResponse.json({ error: "Daemon unreachable" }, { status: 502 });
  }
}
