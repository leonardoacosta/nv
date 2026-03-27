import { NextResponse } from "next/server";
import { count, desc, eq, ilike, like, max } from "drizzle-orm";
import { db } from "@/lib/db";
import {
  projects,
  messages,
  sessions,
  obligations,
  memory,
  diary,
} from "@nova/db";
import type { ProjectExtractionResponse } from "@/types/api";

/**
 * Extract and assemble knowledge documents for all projects.
 *
 * For each project, runs parallel queries across messages, sessions,
 * obligations, memory, and diary tables, then assembles structured
 * markdown and updates the project content + description columns.
 */
export async function POST() {
  try {
    const allProjects = await db.select().from(projects);

    if (allProjects.length === 0) {
      const response: ProjectExtractionResponse = {
        projects_updated: 0,
        sources_scanned: [],
      };
      return NextResponse.json(response);
    }

    const sourcesSet = new Set<string>();
    let updatedCount = 0;

    for (const project of allProjects) {
      const code = project.code;
      const codePattern = `%${code}%`;

      // Run all queries in parallel
      const [
        messageStats,
        sessionStats,
        obligationStats,
        memoryTopics,
        diaryStats,
        topSenders,
      ] = await Promise.all([
        // (a) Message count + last mention
        db
          .select({
            messageCount: count(messages.id),
            lastMention: max(messages.createdAt),
          })
          .from(messages)
          .where(ilike(messages.content, codePattern))
          .then((rows) => rows[0] ?? { messageCount: 0, lastMention: null }),

        // (b) Session count + last startedAt
        db
          .select({
            sessionCount: count(sessions.id),
            lastStarted: max(sessions.startedAt),
          })
          .from(sessions)
          .where(eq(sessions.project, code))
          .then((rows) => rows[0] ?? { sessionCount: 0, lastStarted: null }),

        // (c) Obligation counts grouped by status
        db
          .select({
            status: obligations.status,
            total: count(obligations.id),
          })
          .from(obligations)
          .where(eq(obligations.projectCode, code))
          .groupBy(obligations.status),

        // (d) Memory topics matching project
        db
          .select({ topic: memory.topic, content: memory.content })
          .from(memory)
          .where(like(memory.topic, "projects-%")),

        // (e) Diary count + last entry
        db
          .select({
            diaryCount: count(diary.id),
            lastEntry: max(diary.createdAt),
          })
          .from(diary)
          .where(ilike(diary.content, codePattern))
          .then((rows) => rows[0] ?? { diaryCount: 0, lastEntry: null }),

        // (f) Top 10 distinct senders from messages mentioning this project
        db
          .select({
            sender: messages.sender,
            msgCount: count(messages.id),
          })
          .from(messages)
          .where(ilike(messages.content, codePattern))
          .groupBy(messages.sender)
          .orderBy(desc(count(messages.id)))
          .limit(10),
      ]);

      // Track sources
      if (messageStats.messageCount > 0) sourcesSet.add("messages");
      if (sessionStats.sessionCount > 0) sourcesSet.add("sessions");
      if (obligationStats.length > 0) sourcesSet.add("obligations");
      if (diaryStats.diaryCount > 0) sourcesSet.add("diary");

      // Filter memory topics relevant to this project
      const relevantMemory = memoryTopics.filter((t) =>
        t.content.toLowerCase().includes(code.toLowerCase()),
      );
      if (relevantMemory.length > 0) sourcesSet.add("memory");

      // Build obligation summary
      const oblSummary: Record<string, number> = {};
      for (const row of obligationStats) {
        oblSummary[row.status] = row.total;
      }
      const totalObligations = Object.values(oblSummary).reduce((a, b) => a + b, 0);

      // Build top senders list
      const sendersList = topSenders
        .filter((s) => s.sender)
        .map((s) => `- ${s.sender} (${s.msgCount} messages)`)
        .join("\n");

      // Build memory excerpt
      const memoryExcerpt = relevantMemory
        .slice(0, 3)
        .map((t) => `### ${t.topic}\n${t.content.slice(0, 300)}`)
        .join("\n\n");

      // Assemble structured markdown
      const contentParts: string[] = [
        `# Project: ${project.name} (${code})`,
        "",
        "## Activity Summary",
        "",
        `- **Messages mentioning project:** ${messageStats.messageCount}${messageStats.lastMention ? ` (last: ${new Date(messageStats.lastMention).toISOString().split("T")[0]})` : ""}`,
        `- **Sessions:** ${sessionStats.sessionCount}${sessionStats.lastStarted ? ` (last: ${new Date(sessionStats.lastStarted).toISOString().split("T")[0]})` : ""}`,
        `- **Obligations:** ${totalObligations} total${oblSummary["open"] ? `, ${oblSummary["open"]} open` : ""}${oblSummary["in_progress"] ? `, ${oblSummary["in_progress"]} in progress` : ""}${oblSummary["done"] ? `, ${oblSummary["done"]} done` : ""}`,
        `- **Diary entries:** ${diaryStats.diaryCount}${diaryStats.lastEntry ? ` (last: ${new Date(diaryStats.lastEntry).toISOString().split("T")[0]})` : ""}`,
      ];

      if (sendersList) {
        contentParts.push("", "## Top Contributors", "", sendersList);
      }

      if (memoryExcerpt) {
        contentParts.push("", "## Memory Context", "", memoryExcerpt);
      }

      const content = contentParts.join("\n");

      // Build a short description
      const descParts: string[] = [];
      if (sessionStats.sessionCount > 0) {
        descParts.push(`${sessionStats.sessionCount} sessions`);
      }
      if (totalObligations > 0) {
        descParts.push(`${totalObligations} obligations`);
      }
      if (messageStats.messageCount > 0) {
        descParts.push(`${messageStats.messageCount} messages`);
      }
      const description = descParts.length > 0
        ? descParts.join(", ")
        : null;

      // Update project row
      await db
        .update(projects)
        .set({
          content,
          description,
          updatedAt: new Date(),
        })
        .where(eq(projects.id, project.id));

      updatedCount++;
    }

    const response: ProjectExtractionResponse = {
      projects_updated: updatedCount,
      sources_scanned: Array.from(sourcesSet),
    };
    return NextResponse.json(response);
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
