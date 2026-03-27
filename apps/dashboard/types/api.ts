/**
 * Canonical TypeScript response types for the Nova dashboard API.
 *
 * Data sources:
 * - DB-backed routes: Drizzle queries against @nova/db schemas
 * - Fleet-backed routes: HTTP calls to fleet microservices (tool-router, memory-svc, messages-svc, meta-svc)
 * - Static routes: Environment variables / config
 */

// ── GET /api/memory ────────────────────────────────────────────────────────

/** Response when no `topic` query param is provided — returns list of topic names. */
export interface MemoryListResponse {
  topics: string[];
}

/** Response when `?topic=<name>` is provided — returns the file content. */
export interface MemoryTopicResponse {
  topic: string;
  content: string;
}

// ── PUT /api/memory ────────────────────────────────────────────────────────

export interface PutMemoryRequest {
  topic: string;
  content: string;
}

export interface PutMemoryResponse {
  topic: string;
  written: number;
}

// ── GET /api/obligations ───────────────────────────────────────────────────

/**
 * A single note on an obligation (from obligation_notes table).
 */
export interface ObligationNote {
  id: string;
  obligation_id: string;
  /** "execution_result" | "research" | "comment" | string */
  note_type: string;
  content: string;
  created_at: string;
}

/**
 * A single activity event derived from obligation update history.
 */
export interface ObligationActivity {
  id: string;
  event_type: string;
  obligation_id: string;
  description: string;
  timestamp: string;
  metadata?: Record<string, unknown>;
}

/**
 * Stats summary returned by GET /api/obligations/stats.
 * Computed via Drizzle aggregation queries on the obligations table.
 */
export interface ObligationStats {
  open_nova: number;
  open_leo: number;
  in_progress: number;
  proposed_done: number;
  done_today: number;
}

/**
 * A single obligation returned by GET /api/obligations.
 * Field names use snake_case to match the original API contract.
 * Source: Drizzle query on obligations table.
 */
export interface DaemonObligation {
  id: string;
  source_channel: string;
  source_message: string | null;
  detected_action: string;
  project_code: string | null;
  priority: number;
  /** "open" | "in_progress" | "proposed_done" | "done" | "dismissed" */
  status: string;
  /** "nova" | "leo" */
  owner: string;
  owner_reason: string | null;
  deadline: string | null;
  created_at: string;
  updated_at: string;
  /** Execution notes (from obligation_notes table), newest first */
  notes: ObligationNote[];
  /** Number of execution attempts */
  attempt_count: number;
  /** ISO timestamp of last attempt, if any */
  last_attempt_at: string | null;
}

export interface ObligationsGetResponse {
  obligations: DaemonObligation[];
}

export interface ObligationActivityGetResponse {
  events: ObligationActivity[];
}

// ── GET /api/projects ──────────────────────────────────────────────────────

export interface ApiProject {
  code: string;
  path: string;
}

export interface ProjectsGetResponse {
  projects: ApiProject[];
}

// ── GET /api/sessions ─────────────────────────────────────────────────────

export interface NexusSessionRaw {
  id: string;
  project?: string;
  status: string;
  agent_name: string;
  started_at?: string;
  duration_display: string;
  branch?: string;
  spec?: string;
  progress?: {
    workflow: string;
    phase: string;
    progress_pct: number;
    phase_label: string;
  };
}

/** Session list response from Drizzle query on sessions table. */
export interface SessionsGetResponse {
  sessions: NexusSessionRaw[];
  uptime_secs?: number;
  triggers_processed?: number;
  last_digest_at?: string | null;
}

// ── GET /api/cc-sessions ───────────────────────────────────────────────────

/**
 * Summary of a CC session.
 * Source: Drizzle query on sessions table filtered by command pattern.
 */
export interface CcSessionSummary {
  id: string;
  project: string;
  state: "running" | "completed" | "stopped" | string;
  machine_name: string;
  started_at: string;
  duration_display: string;
  restart_attempts: number;
}

export interface CcSessionsGetResponse {
  sessions: CcSessionSummary[];
  configured: boolean;
}

// ── GET /api/config ────────────────────────────────────────────────────────

/** The config endpoint returns environment-derived configuration. */
export type ConfigGetResponse = Record<string, unknown>;

// ── PUT /api/config ────────────────────────────────────────────────────────

export interface PutConfigRequest {
  fields: Record<string, unknown>;
}

export interface PutConfigResponse {
  applied: string[];
  note: string;
}

// ── GET /api/server-health ────────────────────────────────────────────────

/** Health status enum. */
export type BackendHealthStatus = "healthy" | "degraded" | "critical";

/** A single server health snapshot. */
export interface ServerHealthSnapshot {
  id: number;
  timestamp: string;
  cpu_percent: number | null;
  memory_used_mb: number | null;
  memory_total_mb: number | null;
  disk_used_gb: number | null;
  disk_total_gb: number | null;
  uptime_seconds: number | null;
  load_avg_1m: number | null;
  load_avg_5m: number | null;
}

export interface ServerHealthGetResponse {
  daemon: Record<string, unknown>;
  latest: ServerHealthSnapshot | null;
  status: BackendHealthStatus;
  history: ServerHealthSnapshot[];
}

// ── GET /api/briefing ──────────────────────────────────────────────────────

export interface BriefingAction {
  id: string;
  label: string;
  status: "pending" | "completed" | "dismissed";
}

export interface BriefingEntry {
  id: string;
  generated_at: string;
  content: string;
  suggested_actions: BriefingAction[];
  sources_status: Record<string, string>;
}

/** Response from GET /api/briefing — returns latest entry (or null when no briefing exists). */
export interface BriefingGetResponse {
  entry: BriefingEntry | null;
}

/** Response from GET /api/briefing/history — returns list of past entries. */
export interface BriefingHistoryGetResponse {
  entries: BriefingEntry[];
}

// ── GET /api/messages ──────────────────────────────────────────────────────

/** A single stored message. Source: messages-svc fleet service. */
export interface StoredMessage {
  id: number;
  timestamp: string;
  direction: string;
  channel: string;
  sender: string;
  content: string;
  response_time_ms: number | null;
  tokens_in: number | null;
  tokens_out: number | null;
}

export interface MessagesGetResponse {
  messages: StoredMessage[];
  limit: number;
  offset: number;
}

// ── POST /api/chat/send ───────────────────────────────────────────────────

export interface ChatSendRequest {
  message: string;
}

export interface ChatSSEChunk {
  type: "chunk";
  text: string;
}

export interface ChatSSEDone {
  type: "done";
  full_text: string;
}

export interface ChatSSEError {
  type: "error";
  message: string;
}

export type ChatSSEEvent = ChatSSEChunk | ChatSSEDone | ChatSSEError;

// ── GET /api/activity-feed ────────────────────────────────────────────────

/** A single event in the unified activity feed (messages + obligations + diary). */
export interface ActivityFeedEvent {
  id: string;
  type: "message" | "obligation" | "diary";
  timestamp: string;
  icon_hint: string;
  summary: string;
}

/** Response from GET /api/activity-feed. */
export interface ActivityFeedGetResponse {
  events: ActivityFeedEvent[];
}

// ── GET /stats ─────────────────────────────────────────────────────────────

/** Per-tool breakdown entry. Source: meta-svc fleet service. */
export interface ToolBreakdown {
  name: string;
  count: number;
  success_count: number;
  avg_duration_ms: number | null;
}

/** Aggregated tool usage from meta-svc. */
export interface ToolStatsReport {
  total_invocations: number;
  invocations_today: number;
  per_tool: ToolBreakdown[];
}

/** The `/stats` endpoint returns tool usage stats from the fleet. */
export interface StatsGetResponse {
  tool_usage: ToolStatsReport;
  [key: string]: unknown;
}

// ── GET /api/contacts ──────────────────────────────────────────────────────

/**
 * A single contact. Source: Drizzle query on contacts table.
 */
export interface Contact {
  id: string;
  name: string;
  channel_ids: {
    telegram?: string;
    discord?: string;
    teams?: string;
    [key: string]: string | undefined;
  };
  relationship_type: "work" | "personal-client" | "contributor" | "social";
  notes: string | null;
  created_at: string;
  updated_at: string;
}

// ── GET /api/diary ─────────────────────────────────────────────────────────

/** A single diary entry. Source: Drizzle query on diary table. */
export interface DiaryEntryItem {
  time: string;
  trigger_type: string;
  trigger_source: string;
  channel_source: string;
  slug: string;
  tools_called: string[];
  result_summary: string;
  response_latency_ms: number;
  tokens_in: number;
  tokens_out: number;
}

/** Response from GET /api/diary. */
export interface DiaryGetResponse {
  date: string;
  entries: DiaryEntryItem[];
  total: number;
}
