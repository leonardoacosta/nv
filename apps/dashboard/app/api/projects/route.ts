import { NextResponse } from "next/server";

export async function GET() {
  return NextResponse.json(
    { error: "Not implemented — daemon endpoint pending" },
    { status: 501 },
  );
}
