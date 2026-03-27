import { fleetGet, fleetPost } from "../../fleet-client.js";

const TELEGRAM_MAX_CHARS = 4000;
const GRAPH_SVC_PORT = 4107;

interface MailMessage {
  subject?: string;
  from?: string | { emailAddress?: { name?: string; address?: string } };
  receivedDateTime?: string;
  bodyPreview?: string;
  id?: string;
}

function formatSender(from?: string | { emailAddress?: { name?: string; address?: string } }): string {
  if (!from) return "Unknown";
  if (typeof from === "string") return from;
  const addr = from.emailAddress;
  if (!addr) return "Unknown";
  return addr.name ?? addr.address ?? "Unknown";
}

function formatDate(raw?: string): string {
  if (!raw) return "?";
  try {
    const d = new Date(raw);
    return `${d.toLocaleDateString()} ${d.toTimeString().slice(0, 5)}`;
  } catch {
    return raw.slice(0, 16);
  }
}

/**
 * /mail — Outlook email commands via graph-svc
 *
 * Subcommands:
 *   (no args)       — recent inbox
 *   inbox            — recent inbox
 *   read <id>        — read full email
 *   search <query>   — search emails
 */
export async function buildMailReply(subcommand?: string, arg?: string): Promise<string> {
  if (!subcommand || subcommand === "inbox") {
    return buildInboxReply();
  }

  if (subcommand === "read") {
    if (!arg) return "Usage: /mail read <message_id>";
    return buildReadReply(arg);
  }

  if (subcommand === "search") {
    if (!arg) return "Usage: /mail search <query>";
    return buildSearchMailReply(arg);
  }

  return `Unknown subcommand: ${subcommand}\nUsage: /mail, /mail read <id>, /mail search <query>`;
}

async function buildInboxReply(): Promise<string> {
  const data = await fleetGet(GRAPH_SVC_PORT, "/mail/inbox?limit=10");
  const result = extractResult(data);

  if (typeof result === "string") {
    return truncate(`Inbox\n${"─".repeat(32)}\n${result}`);
  }

  const messages = extractMessages(result);
  if (messages.length === 0) {
    return "No emails in inbox.";
  }

  const header = `Inbox (${messages.length} emails)\n${"─".repeat(32)}\n`;
  const lines = messages.map((m) => {
    const subject = m.subject ?? "(no subject)";
    const sender = formatSender(m.from);
    const date = formatDate(m.receivedDateTime);
    const id = m.id ? `  ID: ${m.id}` : "";
    return `  ${date} ${sender}\n    ${subject}${id}`;
  });

  return truncate(header + lines.join("\n\n"));
}

async function buildReadReply(messageId: string): Promise<string> {
  const data = await fleetGet(GRAPH_SVC_PORT, `/mail/read/${encodeURIComponent(messageId)}`);
  const result = extractResult(data);

  if (typeof result === "string") {
    return truncate(result);
  }

  const msg = result as MailMessage;
  const subject = msg.subject ?? "(no subject)";
  const sender = formatSender(msg.from);
  const date = formatDate(msg.receivedDateTime);
  const body = msg.bodyPreview ?? "(no content)";

  return truncate(
    `${subject}\n${"─".repeat(32)}\nFrom: ${sender}\nDate: ${date}\n\n${body}`,
  );
}

async function buildSearchMailReply(query: string): Promise<string> {
  const data = await fleetPost(GRAPH_SVC_PORT, "/mail/search", { query, limit: 10 });
  const result = extractResult(data);

  if (typeof result === "string") {
    return truncate(`Search: "${query}"\n${"─".repeat(32)}\n${result}`);
  }

  const messages = extractMessages(result);
  if (messages.length === 0) {
    return `No emails found for "${query}".`;
  }

  const header = `Search: "${query}" (${messages.length} results)\n${"─".repeat(32)}\n`;
  const lines = messages.map((m) => {
    const subject = m.subject ?? "(no subject)";
    const sender = formatSender(m.from);
    const date = formatDate(m.receivedDateTime);
    return `  ${date} ${sender}\n    ${subject}`;
  });

  return truncate(header + lines.join("\n\n"));
}

function extractResult(data: unknown): unknown {
  if (data && typeof data === "object" && "result" in data) {
    return (data as { result: unknown }).result;
  }
  return data;
}

function extractMessages(data: unknown): MailMessage[] {
  if (Array.isArray(data)) return data as MailMessage[];
  if (data && typeof data === "object") {
    const obj = data as Record<string, unknown>;
    if (Array.isArray(obj["messages"])) return obj["messages"] as MailMessage[];
    if (Array.isArray(obj["value"])) return obj["value"] as MailMessage[];
  }
  return [];
}

function truncate(text: string): string {
  if (text.length <= TELEGRAM_MAX_CHARS) return text;
  return text.slice(0, TELEGRAM_MAX_CHARS - 3) + "...";
}
