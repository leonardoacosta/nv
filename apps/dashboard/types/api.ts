/**
 * Canonical TypeScript response types for the NV daemon API.
 *
 * All types are derived from the Axum handler definitions in
 * `crates/nv-daemon/src/dashboard.rs` and `crates/nv-daemon/src/http.rs`.
 * Do not guess shapes — read the backend before adding types here.
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
 * Matches `ObligationNote` struct in nv-daemon.
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
 * A single activity event from the obligation activity ring buffer.
 * Matches `ObligationActivityEvent` in nv-daemon/src/http.rs.
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
 * Matches `ObligationStats` in nv-daemon/src/obligation_store.rs.
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
 * Field names match the Rust `Obligation` struct in nv-core/src/types.rs.
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

/** Both the Nexus path and the fallback channel-proxy path return this wrapper. */
export interface SessionsGetResponse {
  sessions: NexusSessionRaw[];
  uptime_secs?: number;
  triggers_processed?: number;
  last_digest_at?: string | null;
}

// ── GET /api/cc-sessions ───────────────────────────────────────────────────

/**
 * Summary of a CC subprocess session managed by CcSessionManager.
 * Matches `CcSessionSummary` in `crates/nv-daemon/src/cc_sessions.rs`.
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

/** The config endpoint returns the raw config JSON — shape varies by project. */
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

/** Backend `HealthStatus` enum serializes as snake_case strings. */
export type BackendHealthStatus = "healthy" | "degraded" | "critical";

/** A single server health snapshot (from `ServerHealthSnapshot` in Rust). */
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

/** Response from GET /api/briefing — returns latest entry or 404. */
export interface BriefingGetResponse {
  entry: BriefingEntry;
}

/** Response from GET /api/briefing/history — returns list of past entries. */
export interface BriefingHistoryGetResponse {
  entries: BriefingEntry[];
}

// ── GET /api/messages ──────────────────────────────────────────────────────

/** A single stored message from the daemon message store. */
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

// ── GET /stats ─────────────────────────────────────────────────────────────

/** Per-tool breakdown entry from `ToolBreakdown` in Rust. */
export interface ToolBreakdown {
  name: string;
  count: number;
  success_count: number;
  avg_duration_ms: number | null;
}

/** Aggregated tool usage from `ToolStatsReport` in Rust. */
export interface ToolStatsReport {
  total_invocations: number;
  invocations_today: number;
  per_tool: ToolBreakdown[];
}

/** The `/stats` endpoint merges message stats, tool_usage, claude_usage, and budget. */
export interface StatsGetResponse {
  tool_usage: ToolStatsReport;
  [key: string]: unknown;
}

// ── GET /api/contacts ──────────────────────────────────────────────────────

/**
 * A single contact returned by GET /api/contacts.
 * Field names match the Rust `Contact` struct in crates/nv-daemon/src/contact_store.rs.
 * The GET /api/contacts handler returns Vec<Contact> as a plain JSON array.
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

/** A single diary entry returned by GET /api/diary. */
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
