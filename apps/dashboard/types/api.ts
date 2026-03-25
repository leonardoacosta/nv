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
