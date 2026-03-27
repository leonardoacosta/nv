import type { Pool } from "pg";
import type { Logger } from "pino";
import { fleetGet } from "../../fleet-client.js";

// ─── Types ────────────────────────────────────────────────────────────────────

export type SourceStatus = "ok" | "unavailable" | "empty";

export interface GatherDeps {
  pool: Pool;
  logger: Logger;
}

export interface EmailItem {
  id: string;
  from: string;
  subject: string;
  preview: string;
  receivedAt: string;
}

export interface TeamsChatItem {
  id: string;
  chatType: string; // "oneOnOne" | "group" | "meeting"
  topic: string;
  lastMessage: string;
  lastMessageFrom: string;
  lastMessageAt: string;
  mentioned: boolean;
}

export interface CalendarItem {
  id: string;
  subject: string;
  start: string;
  end: string;
  isAllDay: boolean;
  location: string;
}

export interface PimRoleItem {
  id: string;
  roleName: string;
  status: string; // "active" | "eligible"
  expiresAt: string | null;
}

export interface AdoBuildItem {
  id: string;
  pipeline: string;
  result: string; // "succeeded" | "failed" | "canceled" | "partiallySucceeded"
  finishTime: string;
  sourceBranch: string;
}

export interface ObligationItem {
  id: string;
  action: string;
  owner: string;
  status: string;
  priority: number;
  projectCode: string | null;
  deadline: Date | null;
  createdAt: Date;
}

export interface GatherResult {
  emails: EmailItem[];
  teamsChats: TeamsChatItem[];
  calendar: CalendarItem[];
  pimRoles: PimRoleItem[];
  adoBuilds: AdoBuildItem[];
  obligations: ObligationItem[];
  sourcesStatus: Record<string, SourceStatus>;
}

// ─── Parsers ──────────────────────────────────────────────────────────────────

/**
 * Parse fleet response text into structured items.
 * Fleet services return `{ result: string }` with human-readable summaries.
 * We do best-effort parsing; unparseable data becomes a single raw item.
 */
function parseEmails(raw: string): EmailItem[] {
  if (!raw || raw.trim() === "" || /no\s+unread/i.test(raw)) return [];

  const items: EmailItem[] = [];
  // Each email is typically rendered as lines — parse what we can
  const blocks = raw.split(/\n(?=From:|Subject:)/i);

  for (const block of blocks) {
    const fromMatch = /from:\s*(.+)/i.exec(block);
    const subjectMatch = /subject:\s*(.+)/i.exec(block);
    const idMatch = /id:\s*(.+)/i.exec(block);

    if (fromMatch || subjectMatch) {
      items.push({
        id: idMatch?.[1]?.trim() ?? `email-${items.length}`,
        from: fromMatch?.[1]?.trim() ?? "unknown",
        subject: subjectMatch?.[1]?.trim() ?? "(no subject)",
        preview: block.slice(0, 200).trim(),
        receivedAt: new Date().toISOString(),
      });
    }
  }

  // If parsing yielded nothing but raw text exists, treat entire text as one item
  if (items.length === 0 && raw.trim().length > 0) {
    items.push({
      id: "email-raw-0",
      from: "unknown",
      subject: raw.slice(0, 100).trim(),
      preview: raw.slice(0, 200).trim(),
      receivedAt: new Date().toISOString(),
    });
  }

  return items;
}

function parseTeamsChats(raw: string): TeamsChatItem[] {
  if (!raw || raw.trim() === "" || /no\s+(recent\s+)?chats/i.test(raw)) return [];

  const items: TeamsChatItem[] = [];
  const lines = raw.split("\n").filter((l) => l.trim().length > 0);

  for (const line of lines) {
    const dmMatch = /\[(DM|1:1|oneOnOne)\]/i.exec(line);
    const mentionMatch = /@mention/i.test(line);
    const chatType = dmMatch ? "oneOnOne" : "group";

    items.push({
      id: `teams-${items.length}`,
      chatType,
      topic: line.slice(0, 100).trim(),
      lastMessage: line.trim(),
      lastMessageFrom: "unknown",
      lastMessageAt: new Date().toISOString(),
      mentioned: mentionMatch,
    });
  }

  return items;
}

function parseCalendar(raw: string): CalendarItem[] {
  if (!raw || raw.trim() === "" || /no\s+events/i.test(raw)) return [];

  const items: CalendarItem[] = [];
  const lines = raw.split("\n").filter((l) => l.trim().length > 0);

  for (const line of lines) {
    const timeMatch = /(\d{1,2}:\d{2})\s*-\s*(\d{1,2}:\d{2})/.exec(line);
    items.push({
      id: `cal-${items.length}`,
      subject: line.replace(/\d{1,2}:\d{2}\s*-\s*\d{1,2}:\d{2}/, "").trim() || line.trim(),
      start: timeMatch?.[1] ?? "",
      end: timeMatch?.[2] ?? "",
      isAllDay: !timeMatch,
      location: "",
    });
  }

  return items;
}

function parsePimRoles(raw: string): PimRoleItem[] {
  if (!raw || raw.trim() === "" || /no\s+roles/i.test(raw)) return [];

  const items: PimRoleItem[] = [];
  const lines = raw.split("\n").filter((l) => l.trim().length > 0);

  for (const line of lines) {
    const activeMatch = /\bactive\b/i.test(line);
    const expiresMatch = /expires?\s*(?:at|in|:)?\s*(.+)/i.exec(line);

    items.push({
      id: `pim-${items.length}`,
      roleName: line.replace(/\(.*?\)/g, "").trim().slice(0, 100),
      status: activeMatch ? "active" : "eligible",
      expiresAt: expiresMatch?.[1]?.trim() ?? null,
    });
  }

  return items;
}

function parseAdoBuilds(raw: string): AdoBuildItem[] {
  if (!raw || raw.trim() === "" || /no\s+builds/i.test(raw)) return [];

  const items: AdoBuildItem[] = [];
  const lines = raw.split("\n").filter((l) => l.trim().length > 0);

  for (const line of lines) {
    const failedMatch = /\bfailed\b/i.test(line);
    const succeededMatch = /\bsucceeded\b/i.test(line);
    const branchMatch = /\b(main|master|release|prod)/i.exec(line);

    const result = failedMatch
      ? "failed"
      : succeededMatch
        ? "succeeded"
        : "unknown";

    items.push({
      id: `ado-${items.length}`,
      pipeline: line.slice(0, 100).trim(),
      result,
      finishTime: new Date().toISOString(),
      sourceBranch: branchMatch?.[1] ?? "unknown",
    });
  }

  return items;
}

// ─── Gather ───────────────────────────────────────────────────────────────────

interface ObligationRow {
  id: string;
  detected_action: string;
  owner: string;
  status: string;
  priority: number;
  project_code: string | null;
  deadline: Date | null;
  created_at: Date;
}

export async function gatherDigest(deps: GatherDeps): Promise<GatherResult> {
  const { pool, logger } = deps;

  const [emailResult, teamsResult, calendarResult, pimResult, adoResult, obResult] =
    await Promise.allSettled([
      fleetGet(4107, "/mail/unread"),
      fleetGet(4105, "/chats"),
      fleetGet(4107, "/calendar/today"),
      fleetGet(4107, "/pim/status", 30000),
      fleetGet(4107, "/ado/builds", 30000),
      pool.query<ObligationRow>(
        `SELECT id, detected_action, owner, status, priority, project_code, deadline, created_at
         FROM obligations
         WHERE status IN ('pending', 'in_progress')
         ORDER BY priority ASC, created_at ASC
         LIMIT 20`,
      ),
    ]);

  const sourcesStatus: Record<string, SourceStatus> = {};

  // Parse emails
  let emails: EmailItem[] = [];
  if (emailResult.status === "fulfilled") {
    const data = emailResult.value as { result?: string };
    const raw = data?.result ?? "";
    emails = parseEmails(raw);
    sourcesStatus["email"] = emails.length === 0 ? "empty" : "ok";
  } else {
    logger.warn({ err: emailResult.reason }, "Digest: email fetch failed");
    sourcesStatus["email"] = "unavailable";
  }

  // Parse Teams chats
  let teamsChats: TeamsChatItem[] = [];
  if (teamsResult.status === "fulfilled") {
    const data = teamsResult.value as { result?: string };
    const raw = data?.result ?? "";
    teamsChats = parseTeamsChats(raw);
    sourcesStatus["teams"] = teamsChats.length === 0 ? "empty" : "ok";
  } else {
    logger.warn({ err: teamsResult.reason }, "Digest: teams fetch failed");
    sourcesStatus["teams"] = "unavailable";
  }

  // Parse calendar
  let calendar: CalendarItem[] = [];
  if (calendarResult.status === "fulfilled") {
    const data = calendarResult.value as { result?: string };
    const raw = data?.result ?? "";
    calendar = parseCalendar(raw);
    sourcesStatus["calendar"] = calendar.length === 0 ? "empty" : "ok";
  } else {
    logger.warn({ err: calendarResult.reason }, "Digest: calendar fetch failed");
    sourcesStatus["calendar"] = "unavailable";
  }

  // Parse PIM roles
  let pimRoles: PimRoleItem[] = [];
  if (pimResult.status === "fulfilled") {
    const data = pimResult.value as { result?: string };
    const raw = data?.result ?? "";
    pimRoles = parsePimRoles(raw);
    sourcesStatus["pim"] = pimRoles.length === 0 ? "empty" : "ok";
  } else {
    logger.warn({ err: pimResult.reason }, "Digest: PIM fetch failed");
    sourcesStatus["pim"] = "unavailable";
  }

  // Parse ADO builds
  let adoBuilds: AdoBuildItem[] = [];
  if (adoResult.status === "fulfilled") {
    const data = adoResult.value as { result?: string };
    const raw = data?.result ?? "";
    adoBuilds = parseAdoBuilds(raw);
    sourcesStatus["ado"] = adoBuilds.length === 0 ? "empty" : "ok";
  } else {
    logger.warn({ err: adoResult.reason }, "Digest: ADO fetch failed");
    sourcesStatus["ado"] = "unavailable";
  }

  // Parse obligations
  let obligations: ObligationItem[] = [];
  if (obResult.status === "fulfilled") {
    obligations = obResult.value.rows.map((row) => ({
      id: row.id,
      action: row.detected_action,
      owner: row.owner,
      status: row.status,
      priority: row.priority,
      projectCode: row.project_code,
      deadline: row.deadline,
      createdAt: row.created_at,
    }));
    sourcesStatus["obligations"] = obligations.length === 0 ? "empty" : "ok";
  } else {
    logger.warn({ err: obResult.reason }, "Digest: obligations fetch failed");
    sourcesStatus["obligations"] = "unavailable";
  }

  return { emails, teamsChats, calendar, pimRoles, adoBuilds, obligations, sourcesStatus };
}

/**
 * Lightweight gather that only fetches P0-relevant sources (PIM + ADO).
 */
export async function gatherP0Only(deps: GatherDeps): Promise<Pick<GatherResult, "pimRoles" | "adoBuilds" | "sourcesStatus">> {
  const { logger } = deps;

  const [pimResult, adoResult] = await Promise.allSettled([
    fleetGet(4107, "/pim/status", 30000),
    fleetGet(4107, "/ado/builds", 30000),
  ]);

  const sourcesStatus: Record<string, SourceStatus> = {};

  let pimRoles: PimRoleItem[] = [];
  if (pimResult.status === "fulfilled") {
    const data = pimResult.value as { result?: string };
    pimRoles = parsePimRoles(data?.result ?? "");
    sourcesStatus["pim"] = pimRoles.length === 0 ? "empty" : "ok";
  } else {
    logger.warn({ err: pimResult.reason }, "Digest P0: PIM fetch failed");
    sourcesStatus["pim"] = "unavailable";
  }

  let adoBuilds: AdoBuildItem[] = [];
  if (adoResult.status === "fulfilled") {
    const data = adoResult.value as { result?: string };
    adoBuilds = parseAdoBuilds(data?.result ?? "");
    sourcesStatus["ado"] = adoBuilds.length === 0 ? "empty" : "ok";
  } else {
    logger.warn({ err: adoResult.reason }, "Digest P0: ADO fetch failed");
    sourcesStatus["ado"] = "unavailable";
  }

  return { pimRoles, adoBuilds, sourcesStatus };
}
