import { createHash } from "node:crypto";
import type {
  GatherResult,
  EmailItem,
  TeamsChatItem,
  CalendarItem,
  PimRoleItem,
  AdoBuildItem,
  ObligationItem,
} from "./gather.js";

// ─── Types ────────────────────────────────────────────────────────────────────

export type Priority = "P0" | "P1" | "P2";

export interface DigestItem {
  id: string;
  source: string;
  priority: Priority;
  title: string;
  detail: string;
  actionable: boolean;
  sourceId?: string;
}

// ─── Suppression Patterns ─────────────────────────────────────────────────────

const SUPPRESSED_EMAIL_PREFIXES = [
  "noreply@",
  "no-reply@",
  "notifications@",
  "mailer-daemon@",
  "notify@",
  "donotreply@",
  "do-not-reply@",
];

function isAutomatedSender(from: string): boolean {
  const lower = from.toLowerCase();
  return SUPPRESSED_EMAIL_PREFIXES.some((prefix) => lower.includes(prefix));
}

// ─── ID Generation ────────────────────────────────────────────────────────────

function makeItemId(source: string, title: string, detail: string): string {
  const hash = createHash("sha256")
    .update(`${source}:${title}:${detail}`)
    .digest("hex")
    .slice(0, 12);
  return `${source}-${hash}`;
}

// ─── Classification Rules ─────────────────────────────────────────────────────

function classifyEmail(email: EmailItem): DigestItem | null {
  if (isAutomatedSender(email.from)) return null; // Suppress

  const title = email.from;
  const detail = email.subject;
  const id = makeItemId("email", title, detail);

  // Human emails are P1 (from contacts — treated as all humans for now)
  return {
    id,
    source: "email",
    priority: "P1",
    title,
    detail,
    actionable: true,
    sourceId: email.id,
  };
}

function classifyTeamsChat(chat: TeamsChatItem): DigestItem | null {
  const title = chat.topic;
  const detail = chat.lastMessage;
  const id = makeItemId("teams", title, detail);

  // DM (oneOnOne) = P1 — someone is waiting for a reply
  if (chat.chatType === "oneOnOne") {
    return {
      id,
      source: "teams",
      priority: "P1",
      title: `DM: ${chat.lastMessageFrom !== "unknown" ? chat.lastMessageFrom : title}`,
      detail,
      actionable: true,
      sourceId: chat.id,
    };
  }

  // @mentioned in channel = P2
  if (chat.mentioned) {
    return {
      id,
      source: "teams",
      priority: "P2",
      title: `@mention in: ${title}`,
      detail,
      actionable: false,
      sourceId: chat.id,
    };
  }

  // Channel chatter without mention = suppress
  return null;
}

function classifyCalendarEvent(event: CalendarItem): DigestItem {
  const title = event.subject;
  const detail = event.start
    ? `${event.start}${event.end ? ` - ${event.end}` : ""}`
    : "All day";
  const id = makeItemId("calendar", title, detail);

  // Check if event starts within 30 minutes
  let priority: Priority = "P2";
  if (event.start) {
    const now = new Date();
    const [hours, minutes] = event.start.split(":").map(Number);
    if (hours !== undefined && minutes !== undefined) {
      const eventTime = new Date(now);
      eventTime.setHours(hours, minutes, 0, 0);
      const diffMs = eventTime.getTime() - now.getTime();
      if (diffMs > 0 && diffMs <= 30 * 60 * 1000) {
        priority = "P1";
      }
    }
  }

  return {
    id,
    source: "calendar",
    priority,
    title,
    detail,
    actionable: priority === "P1",
    sourceId: event.id,
  };
}

function classifyPimRole(role: PimRoleItem): DigestItem | null {
  const title = role.roleName;
  const id = makeItemId("pim", title, role.status);

  // Active role with expiration within 2 hours = P0
  if (role.status === "active" && role.expiresAt) {
    const expiresAt = new Date(role.expiresAt);
    const diffMs = expiresAt.getTime() - Date.now();

    if (diffMs > 0 && diffMs <= 2 * 60 * 60 * 1000) {
      const minutesLeft = Math.floor(diffMs / 60_000);
      const hoursLeft = Math.floor(minutesLeft / 60);
      const minsRemainder = minutesLeft % 60;
      const timeStr = hoursLeft > 0 ? `${hoursLeft}h ${minsRemainder}m` : `${minsRemainder}m`;

      return {
        id,
        source: "pim",
        priority: "P0",
        title: `Role expires in ${timeStr}`,
        detail: title,
        actionable: true,
        sourceId: role.id,
      };
    }
  }

  // Eligible roles or active roles not expiring soon = P2
  return {
    id,
    source: "pim",
    priority: "P2",
    title: `${role.status}: ${title}`,
    detail: role.expiresAt ?? "no expiry",
    actionable: false,
    sourceId: role.id,
  };
}

function classifyAdoBuild(build: AdoBuildItem): DigestItem | null {
  // Succeeded builds = suppress
  if (build.result === "succeeded") return null;

  const title = build.pipeline;
  const detail = `${build.result} on ${build.sourceBranch}`;
  const id = makeItemId("ado", title, detail);

  // Production pipeline failure = P0
  const isProd = /\b(prod|release|main|master)\b/i.test(build.sourceBranch) ||
    /\b(prod|release)\b/i.test(build.pipeline);

  if (build.result === "failed" && isProd) {
    return {
      id,
      source: "ado",
      priority: "P0",
      title: `Pipeline "${title}" failed`,
      detail: `Branch: ${build.sourceBranch}`,
      actionable: true,
      sourceId: build.id,
    };
  }

  // Non-prod failure = P2
  if (build.result === "failed") {
    return {
      id,
      source: "ado",
      priority: "P2",
      title: `Pipeline "${title}" failed`,
      detail: `Branch: ${build.sourceBranch}`,
      actionable: false,
      sourceId: build.id,
    };
  }

  // Other non-success statuses = P2
  return {
    id,
    source: "ado",
    priority: "P2",
    title: `Pipeline "${title}": ${build.result}`,
    detail: `Branch: ${build.sourceBranch}`,
    actionable: false,
    sourceId: build.id,
  };
}

function classifyObligation(ob: ObligationItem): DigestItem {
  const title = ob.action;
  const detail = ob.owner !== "unknown" ? `Owner: ${ob.owner}` : "";
  const id = makeItemId("obligation", title, ob.id);

  const now = new Date();
  const isOverdue = ob.deadline !== null && ob.deadline.getTime() < now.getTime();
  const isDeadlineToday =
    ob.deadline !== null &&
    ob.deadline.toISOString().slice(0, 10) === now.toISOString().slice(0, 10);

  let priority: Priority = "P2";
  if (isOverdue || isDeadlineToday) {
    priority = "P1";
  }

  return {
    id,
    source: "obligation",
    priority,
    title,
    detail: isOverdue
      ? `OVERDUE - ${detail}`
      : isDeadlineToday
        ? `Due today - ${detail}`
        : detail,
    actionable: true,
    sourceId: ob.id,
  };
}

// ─── Main Classifier ──────────────────────────────────────────────────────────

export function classifyItems(result: GatherResult): DigestItem[] {
  const items: DigestItem[] = [];

  // Classify emails
  for (const email of result.emails) {
    const item = classifyEmail(email);
    if (item) items.push(item);
  }

  // Classify Teams chats
  for (const chat of result.teamsChats) {
    const item = classifyTeamsChat(chat);
    if (item) items.push(item);
  }

  // Classify calendar events
  for (const event of result.calendar) {
    items.push(classifyCalendarEvent(event));
  }

  // Classify PIM roles
  for (const role of result.pimRoles) {
    const item = classifyPimRole(role);
    if (item) items.push(item);
  }

  // Classify ADO builds
  for (const build of result.adoBuilds) {
    const item = classifyAdoBuild(build);
    if (item) items.push(item);
  }

  // Classify obligations
  for (const ob of result.obligations) {
    items.push(classifyObligation(ob));
  }

  // Sort by priority: P0 first, then P1, then P2
  const priorityOrder: Record<Priority, number> = { P0: 0, P1: 1, P2: 2 };
  items.sort((a, b) => priorityOrder[a.priority] - priorityOrder[b.priority]);

  return items;
}

/**
 * Classify only P0 items from PIM and ADO data (for realtime checks).
 */
export function classifyP0Only(
  pimRoles: PimRoleItem[],
  adoBuilds: AdoBuildItem[],
): DigestItem[] {
  const items: DigestItem[] = [];

  for (const role of pimRoles) {
    const item = classifyPimRole(role);
    if (item && item.priority === "P0") items.push(item);
  }

  for (const build of adoBuilds) {
    const item = classifyAdoBuild(build);
    if (item && item.priority === "P0") items.push(item);
  }

  return items;
}
