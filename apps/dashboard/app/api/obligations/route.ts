import { type NextRequest, NextResponse } from "next/server";
import { and, eq, desc } from "drizzle-orm";
import { db } from "@/lib/db";
import { obligations } from "@nova/db";
import { toSnakeCase } from "@/lib/case";

export async function GET(request: NextRequest) {
  try {
    const { searchParams } = request.nextUrl;
    const status = searchParams.get("status");
    const owner = searchParams.get("owner");

    const conditions = [];
    if (status) conditions.push(eq(obligations.status, status));
    if (owner) conditions.push(eq(obligations.owner, owner));

    const where =
      conditions.length === 0
        ? undefined
        : conditions.length === 1
          ? conditions[0]
          : and(...conditions);

    const rows = await db
      .select()
      .from(obligations)
      .where(where)
      .orderBy(desc(obligations.createdAt));

    const mapped = rows.map((row) => ({
      ...toSnakeCase(row as unknown as Record<string, unknown>),
      notes: [],
      attempt_count: 0,
    }));

    return NextResponse.json({ obligations: mapped });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
