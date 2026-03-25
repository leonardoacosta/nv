import { NextResponse } from "next/server";

import { sessionManager } from "@/lib/session-manager";

export async function GET(): Promise<NextResponse> {
  const lines = await sessionManager.getLogs(50);
  return NextResponse.json({ lines });
}
