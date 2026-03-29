import type { ServiceConfig } from "../config.js";
import { sshCloudPC } from "../ssh.js";
import { socksGet, socksPost, socksPatch, isSocksAvailable } from "../socks-client.js";
import { getO365Token, clearO365TokenCache } from "../token-cache.js";
import { sanitize } from "../utils.js";
import { createLogger } from "../logger.js";

const log = createLogger("mail");
const GRAPH_BASE = "https://graph.microsoft.com/v1.0";
const OUTLOOK_SCRIPT = "graph-outlook.ps1";

// ── Types ──────────────────────────────────────────────────────────────

interface GraphMessage {
  id?: string;
  subject?: string;
  from?: { emailAddress?: { name?: string; address?: string } };
  toRecipients?: Array<{ emailAddress?: { name?: string; address?: string } }>;
  receivedDateTime?: string;
  sentDateTime?: string;
  isRead?: boolean;
  bodyPreview?: string;
  body?: { contentType?: string; content?: string };
  flag?: { flagStatus?: string };
  hasAttachments?: boolean;
  importance?: string;
  webLink?: string;
}

interface GraphFolder {
  id?: string;
  displayName?: string;
  totalItemCount?: number;
  unreadItemCount?: number;
}

// ── Formatters ─────────────────────────────────────────────────────────

function formatMessageSummary(msg: GraphMessage, idx: number): string {
  const from = msg.from?.emailAddress?.name ?? msg.from?.emailAddress?.address ?? "Unknown";
  const date = msg.receivedDateTime
    ? new Date(msg.receivedDateTime).toLocaleString("en-US", {
        month: "short", day: "numeric", hour: "numeric", minute: "2-digit",
      })
    : "";
  const unread = msg.isRead === false ? " [UNREAD]" : "";
  const flagged = msg.flag?.flagStatus === "flagged" ? " [FLAGGED]" : "";
  const attachment = msg.hasAttachments ? " [ATTACHMENT]" : "";
  const preview = msg.bodyPreview ? `\n  ${msg.bodyPreview.slice(0, 120)}...` : "";
  return `${idx}. ${msg.subject ?? "(No subject)"}${unread}${flagged}${attachment}\n  From: ${from} | ${date}${preview}\n  ID: ${msg.id ?? ""}`;
}

function formatMessageFull(msg: GraphMessage): string {
  const from = msg.from?.emailAddress?.name ?? msg.from?.emailAddress?.address ?? "Unknown";
  const to = msg.toRecipients?.map((r) => r.emailAddress?.name ?? r.emailAddress?.address).join(", ") ?? "";
  const date = msg.receivedDateTime
    ? new Date(msg.receivedDateTime).toLocaleString("en-US", {
        weekday: "short", month: "short", day: "numeric", year: "numeric",
        hour: "numeric", minute: "2-digit",
      })
    : "";
  // Prefer plain text body, strip HTML tags as fallback
  let body = msg.body?.content ?? msg.bodyPreview ?? "";
  if (msg.body?.contentType === "html") {
    body = body.replace(/<[^>]*>/g, "").replace(/&nbsp;/g, " ").replace(/\s+/g, " ").trim();
  }
  return [
    `Subject: ${msg.subject ?? "(No subject)"}`,
    `From: ${from}`,
    `To: ${to}`,
    `Date: ${date}`,
    msg.hasAttachments ? "Attachments: Yes" : "",
    msg.importance && msg.importance !== "normal" ? `Importance: ${msg.importance}` : "",
    "",
    body,
  ].filter(Boolean).join("\n");
}

function formatMessages(messages: GraphMessage[]): string {
  if (messages.length === 0) return "No messages found.";
  return messages.map((msg, i) => formatMessageSummary(msg, i + 1)).join("\n\n");
}

function formatFolders(folders: GraphFolder[]): string {
  if (folders.length === 0) return "No folders found.";
  return folders
    .map((f) => `${f.displayName ?? "Unknown"} (${f.unreadItemCount ?? 0} unread / ${f.totalItemCount ?? 0} total)\n  ID: ${f.id ?? ""}`)
    .join("\n\n");
}

// ── Helpers ────────────────────────────────────────────────────────────

const MSG_SELECT = "$select=id,subject,from,toRecipients,receivedDateTime,sentDateTime,isRead,bodyPreview,flag,hasAttachments,importance";

async function graphGet(config: ServiceConfig, path: string): Promise<string> {
  const url = `${GRAPH_BASE}${path}`;
  const startMs = Date.now();
  log.info({ path: path.slice(0, 120) }, "Graph GET request");
  const token = await getO365Token(config.cloudpcHost);
  try {
    const result = await socksGet(url, token);
    log.info({ path: path.slice(0, 80), durationMs: Date.now() - startMs, responseBytes: result.length }, "Graph GET completed");
    return result;
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      log.warn({ path: path.slice(0, 80), durationMs: Date.now() - startMs }, "Graph GET 401 — refreshing token");
      clearO365TokenCache();
      const freshToken = await getO365Token(config.cloudpcHost);
      const result = await socksGet(url, freshToken);
      log.info({ path: path.slice(0, 80), durationMs: Date.now() - startMs, responseBytes: result.length }, "Graph GET completed (after token refresh)");
      return result;
    }
    log.error({ path: path.slice(0, 80), durationMs: Date.now() - startMs, error: err instanceof Error ? err.message : String(err) }, "Graph GET failed");
    throw err;
  }
}

async function graphPost(config: ServiceConfig, path: string, body: unknown): Promise<string> {
  const url = `${GRAPH_BASE}${path}`;
  const token = await getO365Token(config.cloudpcHost);
  try {
    return await socksPost(url, token, body);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearO365TokenCache();
      const freshToken = await getO365Token(config.cloudpcHost);
      return await socksPost(url, freshToken, body);
    }
    throw err;
  }
}

async function graphPatch(config: ServiceConfig, path: string, body: unknown): Promise<string> {
  const url = `${GRAPH_BASE}${path}`;
  const token = await getO365Token(config.cloudpcHost);
  try {
    return await socksPatch(url, token, body);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearO365TokenCache();
      const freshToken = await getO365Token(config.cloudpcHost);
      return await socksPatch(url, freshToken, body);
    }
    throw err;
  }
}

// ── Tool implementations ───────────────────────────────────────────────

/**
 * Get recent emails from Outlook inbox.
 */
export async function outlookInbox(
  config: ServiceConfig,
  limit: number = 10,
): Promise<string> {
  log.info({ limit }, "outlook_inbox called");
  if (!(await isSocksAvailable())) {
    return sshCloudPC(config.cloudpcHost, config.cloudpcUserPath, OUTLOOK_SCRIPT, `-Action Inbox -Count ${limit}`);
  }
  const raw = await graphGet(config, `/me/mailFolders/Inbox/messages?$top=${limit}&${MSG_SELECT}&$orderby=receivedDateTime desc`);
  const data = JSON.parse(raw) as { value?: GraphMessage[] };
  log.info({ limit, resultCount: data.value?.length ?? 0 }, "outlook_inbox completed");
  return formatMessages(data.value ?? []);
}

/**
 * Read the full content of an email by message ID.
 */
export async function outlookRead(
  config: ServiceConfig,
  messageId: string,
): Promise<string> {
  log.info({ messageId: messageId.slice(0, 40) }, "outlook_read called");
  if (!(await isSocksAvailable())) {
    log.info({ transport: "ssh" }, "outlook_read via SSH fallback");
    return sshCloudPC(config.cloudpcHost, config.cloudpcUserPath, OUTLOOK_SCRIPT, `-Action Read -MessageId '${sanitize(messageId)}'`);
  }
  const raw = await graphGet(config, `/me/messages/${encodeURIComponent(messageId)}?$select=id,subject,from,toRecipients,receivedDateTime,body,hasAttachments,importance`);
  const msg = JSON.parse(raw) as GraphMessage;
  const hasBody = !!msg.body?.content && msg.body.content.length > 0;
  const hasPreview = !!msg.bodyPreview && msg.bodyPreview.length > 0;
  log.info({
    messageId: messageId.slice(0, 40),
    subject: msg.subject?.slice(0, 80),
    bodyContentType: msg.body?.contentType ?? "none",
    bodyLength: msg.body?.content?.length ?? 0,
    previewLength: msg.bodyPreview?.length ?? 0,
    hasBody,
    hasPreview,
  }, hasBody ? "outlook_read body found" : "outlook_read EMPTY BODY");
  return formatMessageFull(msg);
}

/**
 * Search Outlook emails by keyword.
 */
export async function outlookSearch(
  config: ServiceConfig,
  query: string,
  limit: number = 10,
): Promise<string> {
  log.info({ query: query.slice(0, 80), limit }, "outlook_search called");
  if (!(await isSocksAvailable())) {
    return sshCloudPC(config.cloudpcHost, config.cloudpcUserPath, OUTLOOK_SCRIPT, `-Action Search -Query '${sanitize(query)}' -Count ${limit}`);
  }
  const raw = await graphGet(config, `/me/messages?$search="${encodeURIComponent(query)}"&$top=${limit}&${MSG_SELECT}`);
  const data = JSON.parse(raw) as { value?: GraphMessage[] };
  log.info({ query: query.slice(0, 80), resultCount: data.value?.length ?? 0 }, "outlook_search completed");
  return formatMessages(data.value ?? []);
}

/**
 * List mail folders (Inbox, Sent, Drafts, etc.).
 */
export async function outlookFolders(
  config: ServiceConfig,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    return sshCloudPC(config.cloudpcHost, config.cloudpcUserPath, OUTLOOK_SCRIPT, `-Action Folders`);
  }
  const raw = await graphGet(config, `/me/mailFolders?$select=id,displayName,totalItemCount,unreadItemCount&$top=50`);
  const data = JSON.parse(raw) as { value?: GraphFolder[] };
  return formatFolders(data.value ?? []);
}

/**
 * Get recent sent emails.
 */
export async function outlookSent(
  config: ServiceConfig,
  limit: number = 10,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    return sshCloudPC(config.cloudpcHost, config.cloudpcUserPath, OUTLOOK_SCRIPT, `-Action Sent -Count ${limit}`);
  }
  const raw = await graphGet(config, `/me/mailFolders/SentItems/messages?$top=${limit}&${MSG_SELECT}&$orderby=sentDateTime desc`);
  const data = JSON.parse(raw) as { value?: GraphMessage[] };
  return formatMessages(data.value ?? []);
}

/**
 * Read emails from a specific folder.
 */
export async function outlookFolder(
  config: ServiceConfig,
  folderId: string,
  limit: number = 10,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    return sshCloudPC(config.cloudpcHost, config.cloudpcUserPath, OUTLOOK_SCRIPT, `-Action Folder -FolderId '${sanitize(folderId)}' -Count ${limit}`);
  }
  const raw = await graphGet(config, `/me/mailFolders/${encodeURIComponent(folderId)}/messages?$top=${limit}&${MSG_SELECT}&$orderby=receivedDateTime desc`);
  const data = JSON.parse(raw) as { value?: GraphMessage[] };
  return formatMessages(data.value ?? []);
}

/**
 * Flag an email for follow-up in Outlook.
 */
export async function outlookFlag(
  config: ServiceConfig,
  messageId: string,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    return sshCloudPC(config.cloudpcHost, config.cloudpcUserPath, OUTLOOK_SCRIPT, `-Action Flag -MessageId '${sanitize(messageId)}'`);
  }
  await graphPatch(config, `/me/messages/${encodeURIComponent(messageId)}`, {
    flag: { flagStatus: "flagged" },
  });
  return "Message flagged for follow-up.";
}

/**
 * Move an email to a different Outlook folder.
 */
export async function outlookMove(
  config: ServiceConfig,
  messageId: string,
  destinationFolder: string,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    return sshCloudPC(config.cloudpcHost, config.cloudpcUserPath, OUTLOOK_SCRIPT, `-Action Move -MessageId '${sanitize(messageId)}' -DestinationFolder '${sanitize(destinationFolder)}'`);
  }
  // Graph API move needs the folder ID. Well-known folder names work as destinationId.
  await graphPost(config, `/me/messages/${encodeURIComponent(messageId)}/move`, {
    destinationId: destinationFolder,
  });
  return `Message moved to ${destinationFolder}.`;
}

/**
 * Get unread emails only.
 */
export async function outlookUnread(
  config: ServiceConfig,
  limit: number = 10,
): Promise<string> {
  if (!(await isSocksAvailable())) {
    return sshCloudPC(config.cloudpcHost, config.cloudpcUserPath, OUTLOOK_SCRIPT, `-Action Unread -Count ${limit}`);
  }
  const raw = await graphGet(config, `/me/messages?$filter=isRead eq false&$top=${limit}&${MSG_SELECT}&$orderby=receivedDateTime desc`);
  const data = JSON.parse(raw) as { value?: GraphMessage[] };
  return formatMessages(data.value ?? []);
}
