import { NextResponse } from "next/server";

import { sessionManager } from "@/lib/session-manager";

export async function GET(): Promise<NextResponse> {
  return NextResponse.json(sessionManager.getStatus());
}
