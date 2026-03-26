import { NextResponse } from "next/server";

const NOT_IMPLEMENTED = {
  error: "Not implemented — daemon endpoint pending",
};

export async function GET() {
  return NextResponse.json(NOT_IMPLEMENTED, { status: 501 });
}

export async function PUT() {
  return NextResponse.json(NOT_IMPLEMENTED, { status: 501 });
}
