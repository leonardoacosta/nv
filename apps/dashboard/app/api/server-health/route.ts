import { NextResponse } from "next/server";
import { sql } from "drizzle-orm";
import { db } from "@/lib/db";
import type { ServerHealthGetResponse } from "@/types/api";

export async function GET() {
  try {
    // Verify DB connectivity as a basic health signal
    await db.execute(sql`SELECT 1`);

    const response: ServerHealthGetResponse = {
      daemon: {
        database: { status: "healthy" },
        note: "Fleet service health is monitored by meta-svc on the host. Dashboard queries Postgres directly.",
      },
      latest: null,
      status: "healthy",
      history: [],
    };

    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    const response: ServerHealthGetResponse = {
      daemon: {
        database: { status: "unhealthy", error: message },
      },
      latest: null,
      status: "critical",
      history: [],
    };
    return NextResponse.json(response, { status: 200 });
  }
}
