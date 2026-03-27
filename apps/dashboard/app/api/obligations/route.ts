import { type NextRequest, NextResponse } from "next/server";
import { and, eq, desc } from "drizzle-orm";
import { db } from "@/lib/db";
import { obligations } from "@nova/db";
import { toSnakeCase } from "@/lib/case";

interface CreateObligationBody {
  detected_action: string;
  owner: string;
  status: string;
  priority: number;
  source_channel: string;
}

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

export async function POST(request: NextRequest) {
  try {
    const body = (await request.json()) as Partial<CreateObligationBody>;

    if (!body.detected_action || typeof body.detected_action !== "string" || body.detected_action.trim() === "") {
      return NextResponse.json(
        { error: "detected_action is required and must be a non-empty string" },
        { status: 400 },
      );
    }

    const [row] = await db
      .insert(obligations)
      .values({
        detectedAction: body.detected_action.trim(),
        owner: body.owner ?? "nova",
        status: body.status ?? "open",
        priority: body.priority ?? 2,
        sourceChannel: body.source_channel ?? "dashboard",
      })
      .returning({ id: obligations.id });

    return NextResponse.json({ obligation: { id: row!.id } }, { status: 201 });
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
