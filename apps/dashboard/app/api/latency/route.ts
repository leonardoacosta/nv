import { NextResponse } from "next/server";

export async function GET() {
  return NextResponse.json({
    services: {},
    timestamp: new Date().toISOString(),
    note: "Latency monitoring handled by meta-svc on the host. Dashboard queries Postgres directly.",
  });
}
