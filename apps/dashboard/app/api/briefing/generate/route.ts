import { NextResponse } from "next/server";

const DAEMON_URL =
  process.env["DAEMON_URL"] ?? "http://localhost:7700";

export async function POST() {
  try {
    const res = await fetch(`${DAEMON_URL}/briefing/generate`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
    });

    if (!res.ok) {
      const body = (await res.json().catch(() => ({}))) as {
        error?: string;
      };
      return NextResponse.json(
        { error: body.error ?? `Daemon returned ${res.status}` },
        { status: res.status },
      );
    }

    const data = (await res.json()) as { id: string; generated_at: string };
    return NextResponse.json({
      success: true,
      briefing_id: data.id,
    });
  } catch (err) {
    const message =
      err instanceof Error ? err.message : "Daemon unreachable";
    return NextResponse.json({ error: message }, { status: 503 });
  }
}
