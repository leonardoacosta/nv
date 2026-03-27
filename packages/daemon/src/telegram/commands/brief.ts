import { buildCalendarReply } from "./calendar.js";
import { buildMailReply } from "./mail.js";
import { buildObReply } from "./ob.js";

const TELEGRAM_MAX_CHARS = 4000;

/**
 * /brief — morning briefing combining calendar, mail, and obligations
 */
export async function buildBriefReply(): Promise<string> {
  const [calendarResult, mailResult, obResult] = await Promise.allSettled([
    buildCalendarReply(),
    buildMailReply("inbox"),
    buildObReply(),
  ]);

  const sections: string[] = [
    "Nova Briefing",
    "=".repeat(32),
  ];

  // Calendar
  sections.push("");
  sections.push("Calendar");
  sections.push("-".repeat(32));
  if (calendarResult.status === "fulfilled") {
    sections.push(calendarResult.value);
  } else {
    sections.push("  Calendar unavailable.");
  }

  // Mail
  sections.push("");
  sections.push("Mail");
  sections.push("-".repeat(32));
  if (mailResult.status === "fulfilled") {
    sections.push(mailResult.value);
  } else {
    sections.push("  Mail unavailable.");
  }

  // Obligations
  sections.push("");
  sections.push("Obligations");
  sections.push("-".repeat(32));
  if (obResult.status === "fulfilled") {
    sections.push(obResult.value);
  } else {
    sections.push("  Obligations unavailable.");
  }

  return truncate(sections.join("\n"));
}

function truncate(text: string): string {
  if (text.length <= TELEGRAM_MAX_CHARS) return text;
  return text.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
