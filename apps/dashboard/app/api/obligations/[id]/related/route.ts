import { type NextRequest, NextResponse } from "next/server";
import { and, count, desc, eq, gte, like, lte, ne } from "drizzle-orm";
import { db } from "@/lib/db";
import { obligations, messages, reminders, sessions } from "@nova/db";
import { toSnakeCase } from "@/lib/case";

export async function GET(
  _request: NextRequest,
  { params }: { params: Promise<{ id: string }> },
) {
  try {
    const { id } = await params;

    // [5.1] Load the obligation
    const [obligation] = await db
      .select()
      .from(obligations)
      .where(eq(obligations.id, id))
      .limit(1);

    if (!obligation) {
      return NextResponse.json(
        { error: "Obligation not found" },
        { status: 404 },
      );
    }

    const ONE_HOUR_MS = 3_600_000;

    // [5.2] Find source message via content + time window matching
    let sourceMessage: Record<string, unknown> | null = null;
    if (obligation.sourceMessage && obligation.sourceChannel) {
      const snippet = obligation.sourceMessage.slice(0, 100);
      const windowStart = new Date(
        obligation.createdAt.getTime() - ONE_HOUR_MS,
      );
      const windowEnd = new Date(obligation.createdAt.getTime() + ONE_HOUR_MS);

      const [sourceRow] = await db
        .select()
        .from(messages)
        .where(
          and(
            eq(messages.channel, obligation.sourceChannel),
            like(messages.content, `%${snippet}%`),
            gte(messages.createdAt, windowStart),
            lte(messages.createdAt, windowEnd),
          ),
        )
        .limit(1);

      if (sourceRow) {
        sourceMessage = {
          id: 0,
          timestamp: sourceRow.createdAt.toISOString(),
          direction: sourceRow.sender === "nova" ? "outbound" : "inbound",
          channel: sourceRow.channel ?? "unknown",
          sender: sourceRow.sender ?? "unknown",
          content: sourceRow.content,
          response_time_ms: null,
          tokens_in: null,
          tokens_out: null,
        };
      }
    }

    // [5.3] Project context — obligation count + session count
    let projectContext: {
      code: string;
      obligation_count: number;
      session_count: number;
    } | null = null;

    if (obligation.projectCode) {
      const [oblCount] = await db
        .select({ total: count() })
        .from(obligations)
        .where(eq(obligations.projectCode, obligation.projectCode));

      const [sessCount] = await db
        .select({ total: count() })
        .from(sessions)
        .where(eq(sessions.project, obligation.projectCode));

      projectContext = {
        code: obligation.projectCode,
        obligation_count: oblCount?.total ?? 0,
        session_count: sessCount?.total ?? 0,
      };
    }

    // [5.4] Reminders for this obligation
    const reminderRows = await db
      .select()
      .from(reminders)
      .where(eq(reminders.obligationId, obligation.id));

    const mappedReminders = reminderRows.map((r) => ({
      id: r.id,
      message: r.message,
      due_at: r.dueAt.toISOString(),
      status: r.cancelled
        ? "cancelled"
        : r.deliveredAt
          ? "delivered"
          : "pending",
    }));

    // [5.5] Related obligations in the same project (excluding self)
    let relatedObligations: Record<string, unknown>[] = [];
    if (obligation.projectCode) {
      const relatedRows = await db
        .select()
        .from(obligations)
        .where(
          and(
            eq(obligations.projectCode, obligation.projectCode),
            ne(obligations.id, obligation.id),
          ),
        )
        .orderBy(desc(obligations.createdAt))
        .limit(10);

      relatedObligations = relatedRows.map((row) => ({
        ...toSnakeCase(row as unknown as Record<string, unknown>),
        notes: [],
        attempt_count: 0,
      }));
    }

    // [5.6] Assemble ObligationRelatedResponse
    const mappedObligation = {
      ...toSnakeCase(obligation as unknown as Record<string, unknown>),
      notes: [],
      attempt_count: 0,
    };

    const response = {
      obligation: mappedObligation,
      source_message: sourceMessage,
      project: projectContext,
      reminders: mappedReminders,
      related_obligations: relatedObligations,
    };

    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
