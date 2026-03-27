import { db, contacts } from "@nova/db";
import { asc } from "drizzle-orm";

const TELEGRAM_MAX_CHARS = 4000;

/**
 * /contacts — list all contacts from DB
 */
export async function buildContactsReply(): Promise<string> {
  const rows = await db
    .select()
    .from(contacts)
    .orderBy(asc(contacts.name));

  if (rows.length === 0) {
    return "No contacts found.";
  }

  const header = `Contacts (${rows.length})\n${"─".repeat(32)}\n`;
  const lines = rows.map((c) => {
    const rel = c.relationshipType ? ` [${c.relationshipType}]` : "";
    return `  ${c.name}${rel}`;
  });

  return truncate(header + lines.join("\n"));
}

function truncate(text: string): string {
  if (text.length <= TELEGRAM_MAX_CHARS) return text;
  return text.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
