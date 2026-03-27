import { type NextRequest, NextResponse } from "next/server";
import { db } from "@/lib/db";
import { reminders } from "@nova/db";
import type { CreateReminderRequest } from "@/types/api";

export async function POST(request: NextRequest) {
  try {
    const body = (await request.json()) as CreateReminderRequest;

    // Validate message
    if (!body.message || body.message.trim().length === 0) {
      return NextResponse.json(
        { error: "Message is required and cannot be empty" },
        { status: 400 },
      );
    }

    if (body.message.length > 500) {
      return NextResponse.json(
        { error: "Message must be 500 characters or fewer" },
        { status: 400 },
      );
    }

    // Validate due_at
    if (!body.due_at) {
      return NextResponse.json(
        { error: "due_at is required" },
        { status: 400 },
      );
    }

    const dueAt = new Date(body.due_at);
    if (isNaN(dueAt.getTime())) {
      return NextResponse.json(
        { error: "due_at must be a valid ISO 8601 date" },
        { status: 400 },
      );
    }

    if (dueAt <= new Date()) {
      return NextResponse.json(
        { error: "due_at must be in the future" },
        { status: 400 },
      );
    }

    const channel = body.channel ?? "dashboard";

    const [created] = await db
      .insert(reminders)
      .values({
        message: body.message.trim(),
        dueAt,
        channel,
      })
      .returning();

    return NextResponse.json(
      {
        id: created!.id,
        message: created!.message,
        due_at: created!.dueAt.toISOString(),
        channel: created!.channel,
        created_at: created!.createdAt.toISOString(),
        status: "pending" as const,
      },
      { status: 201 },
    );
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
