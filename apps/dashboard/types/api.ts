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

// ── GET /api/projects (legacy) ────────────────────────────────────────────
// ApiProject and ProjectsGetResponse are superseded by ProjectEntity and
// ProjectsListResponse (see bottom of file). Kept only for backward
// compatibility with entity-resolution and ProjectRelatedResponse.

export interface ApiProject {
  code: string;
  path: string;
}

/** @deprecated Use ProjectsListResponse instead. */
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
  /** Structured blocks from generative UI pipeline. Null for legacy markdown-only entries. */
  blocks: import("@nova/db").BriefingBlock[] | null;
}

/** Response from GET /api/briefing — returns latest entry (or null when no briefing exists). */
export interface BriefingGetResponse {
  entry: BriefingEntry | null;
  /** True when the briefing window has passed today but no briefing was generated. */
  missedToday: boolean;
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
  /** Message type derived from metadata.type JSONB field. Defaults to "conversation". */
  type: "conversation" | "tool-call" | "system";
  /** Resolved sender identity. Populated server-side by message.list. Optional for backward compatibility with code that constructs StoredMessage locally. */
  senderResolved?: {
    displayName: string;
    avatarInitial: string;
    source: "contact" | "telegram-meta" | "memory" | "raw";
  };
}

export interface MessagesGetResponse {
  messages: StoredMessage[];
  /** Total count of matching messages (for "Showing N of M" display). */
  total: number;
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

/** A single event in the unified activity feed (messages + obligations + diary + sessions). */
export interface ActivityFeedEvent {
  id: string;
  type: "message" | "obligation" | "diary" | "session";
  timestamp: string;
  icon_hint: string;
  summary: string;
  severity: "error" | "warning" | "info";
}

/** Response from GET /api/activity-feed. */
export interface ActivityFeedGetResponse {
  events: ActivityFeedEvent[];
}

// ── GET /api/automations ──────────────────────────────────────────────────

export interface AutomationReminder {
  id: string;
  message: string;
  due_at: string;
  channel: string;
  created_at: string;
  status: "pending" | "overdue";
}

export interface AutomationSchedule {
  id: string;
  name: string;
  cron_expr: string;
  action: string;
  channel: string;
  enabled: boolean;
  last_run_at: string | null;
  next_run: string | null;
}

export interface AutomationWatcher {
  enabled: boolean;
  interval_minutes: number;
  quiet_start: string;
  quiet_end: string;
  last_run_at: string | null;
}

export interface AutomationBriefing {
  last_generated_at: string | null;
  next_generation: string | null;
  content_preview: string | null;
  briefing_hour: number;
}

export interface AutomationSession {
  id: string;
  project: string;
  command: string;
  status: string;
  started_at: string;
}

export interface AutomationsGetResponse {
  reminders: AutomationReminder[];
  schedules: AutomationSchedule[];
  watcher: AutomationWatcher;
  briefing: AutomationBriefing;
  active_sessions: AutomationSession[];
}

// ── Settings API ────────────────────────────────────────────────────

export interface AutomationSettingsResponse {
  settings: Record<string, string>;
}

export interface PutSettingRequest {
  key: string;
  value: string;
}

export interface CreateReminderRequest {
  message: string;
  due_at: string;
  channel?: string;
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

// ── GET /api/contacts/discovered ──────────────────────────────────────────

/** A contact auto-discovered from message history, optionally enriched. */
export interface DiscoveredContact {
  name: string;
  channels: string[];
  message_count: number;
  first_seen: string;
  last_seen: string;
  contact_id: string | null;
  relationship_type: string | null;
  notes: string | null;
  channel_ids: Record<string, string> | null;
}

/** Response from GET /api/contacts/discovered. */
export interface DiscoveredContactsResponse {
  contacts: DiscoveredContact[];
  total_senders: number;
  total_messages_scanned: number;
}

// ── Entity resolution types ────────────────────────────────────────────────

/**
 * A parsed person profile from the memory `people` topic.
 * Produced by parsePeopleMemory() in the entity-resolution library.
 */
export interface PersonProfile {
  name: string;
  channel_ids: Record<string, string>;
  role: string | null;
  notes: string;
}

/**
 * Response from GET /api/resolve/senders.
 * Maps "channel:senderId" keys to resolved display names.
 */
export interface SenderResolutionResponse {
  resolutions: Record<string, string>;
  source_counts: {
    contacts_table: number;
    memory_people: number;
    unresolved: number;
  };
}

/**
 * @deprecated Replaced by ProjectEntity. Kept for entity-resolution backward
 * compatibility only. Use ProjectEntity for all new code.
 */
export interface EnrichedProject extends ApiProject {
  description: string | null;
  memory_context: string | null;
  obligation_count: number;
  active_obligation_count: number;
  session_count: number;
  last_activity: string | null;
}

/**
 * Response from GET /api/contacts/:id/related.
 */
export interface ContactRelatedResponse {
  contact: Contact;
  messages: StoredMessage[];
  message_count: number;
  obligations: DaemonObligation[];
  memory_profile: string | null;
  channels_active: string[];
}

/**
 * Response from GET /api/projects/:code/related.
 */
export interface ProjectRelatedResponse {
  project: ApiProject;
  obligations: DaemonObligation[];
  obligation_summary: {
    total: number;
    open: number;
    in_progress: number;
    done: number;
  };
  sessions: NexusSessionRaw[];
  session_count: number;
  memory_topics: Array<{ topic: string; preview: string }>;
  recent_messages: StoredMessage[];
}

/**
 * Response from GET /api/obligations/:id/related.
 */
export interface ObligationRelatedResponse {
  obligation: DaemonObligation;
  source_message: StoredMessage | null;
  project: { code: string; obligation_count: number; session_count: number } | null;
  reminders: Array<{ id: string; message: string; due_at: string; status: string }>;
  related_obligations: DaemonObligation[];
}

// ── GET /api/contacts/relationships ──────────────────────────────────────

/** A relationship edge inferred from message co-occurrence. */
export interface ContactRelationship {
  person_a: string;
  person_b: string;
  shared_channel: string;
  co_occurrence_count: number;
}

/** Response from GET /api/contacts/relationships. */
export interface RelationshipsResponse {
  relationships: ContactRelationship[];
}

// ── GET /api/diary ─────────────────────────────────────────────────────────

/** Structured detail for a single tool call within a diary entry. */
export interface ToolCallDetail {
  name: string;
  input_summary: string;
  duration_ms: number | null;
}

/** A single diary entry. Source: Drizzle query on diary table. */
export interface DiaryEntryItem {
  time: string;
  trigger_type: string;
  trigger_source: string;
  channel_source: string;
  slug: string;
  /** Flat list of tool names (backward compat). */
  tools_called: string[];
  /** Structured tool call details (new format). */
  tools_detail: ToolCallDetail[];
  result_summary: string;
  response_latency_ms: number;
  tokens_in: number;
  tokens_out: number;
  /** Claude model used, e.g. "claude-opus-4-6". Null for legacy entries. */
  model: string | null;
  /** Estimated cost in USD. Null when model is unknown or entry predates cost tracking. */
  cost_usd: number | null;
}

/** Daily aggregate statistics for a diary query window. */
export interface DiaryAggregates {
  total_tokens_in: number;
  total_tokens_out: number;
  /** Total estimated cost in USD, or null if no cost data available. */
  total_cost_usd: number | null;
  /** Average response latency in ms across entries with latency data. */
  avg_latency_ms: number;
  /** Top 10 tools by call frequency, descending. */
  tool_frequency: { name: string; count: number }[];
}

/** Response from GET /api/diary. */
export interface DiaryGetResponse {
  date: string;
  entries: DiaryEntryItem[];
  total: number;
  distinct_channels: number;
  last_interaction_at: string | null;
  aggregates: DiaryAggregates;
}

// ── GET /api/fleet-status ────────────────────────────────────────────────

/** Health status for a single fleet service. */
export interface FleetServiceStatus {
  name: string;
  url: string;
  port: number;
  status: "healthy" | "unreachable" | "unknown";
  latency_ms: number | null;
  tools: string[];
  last_checked: string | null;
  uptime_secs: number | null;
}

/** Fleet health aggregation. */
export interface FleetHealthResponse {
  fleet: {
    status: "healthy" | "degraded" | "unhealthy" | "unknown";
    services: FleetServiceStatus[];
    healthy_count: number;
    total_count: number;
  };
  channels: ChannelStatus[];
}

/** Status of a single channel. */
export interface ChannelStatus {
  name: string;
  status: "configured" | "unknown" | "connected" | "disconnected" | "unconfigured";
  direction: "bidirectional" | "inbound" | "outbound";
  messages_24h: number | null;
  messages_per_hour: number | null;
}

/** Response from tRPC system.errorRates. */
export interface ErrorRateResponse {
  total_24h: number;
  hourly: { hour: string; count: number }[];
  by_type: { event_type: string; count: number }[];
}

/** Response from tRPC system.channelVolume. */
export interface ChannelVolumeResponse {
  channels: {
    name: string;
    total_24h: number;
    hourly: { hour: string; count: number }[];
  }[];
}

/** Response from tRPC system.fleetHistory. */
export interface FleetHistoryResponse {
  services: {
    name: string;
    snapshots: { time: string; status: string; latency_ms: number | null }[];
    uptime_pct_24h: number;
  }[];
}

// ── Automations Prompt Preview (tRPC automation.previewContext) ────────────

/** A single obligation item in the preview context. */
export interface ContextObligationItem {
  id: string;
  detectedAction: string;
  status: string;
  priority: number;
  sourceChannel: string;
  deadline: string | null;
  createdAt: string;
}

/** Obligation section of the preview context. */
export interface ContextObligationSummary {
  status: "ok" | "unavailable" | "empty";
  items: ContextObligationItem[];
  countByStatus: Record<string, number>;
}

/** A single memory item in the preview context. */
export interface ContextMemoryItem {
  topic: string;
  contentPreview: string;
}

/** Messages section of the preview context. */
export interface ContextChannelSummary {
  channel: string;
  count: number;
  latestPreview: string | null;
}

/** Per-channel info for the channel pills UI. */
export interface ChannelInfo {
  name: string;
  messageCount: number;
  active: boolean;
}

/** Stats summary in the preview context. */
export interface ContextStats {
  totalObligations: number;
  activeReminders: number;
  memoryTopics: number;
}

/** Top-level response from tRPC automation.previewContext. */
export interface AutomationContextPreview {
  obligations: ContextObligationSummary;
  memory: {
    status: "ok" | "unavailable" | "empty";
    items: ContextMemoryItem[];
  };
  messages: {
    status: "ok" | "unavailable" | "empty";
    byChannel: ContextChannelSummary[];
  };
  channels: ChannelInfo[];
  stats: ContextStats;
  assembledAt: string;
}

// ── GET /api/sessions/analytics ───────────────────────────────────────────

/** Daily session count entry used in the sessions_7d sparkline. */
export interface SessionDailyCount {
  date: string;
  count: number;
}

/** Project session count entry for the breakdown chart. */
export interface SessionProjectBreakdown {
  project: string;
  count: number;
}

/** Response from GET /api/sessions/analytics. */
export interface SessionAnalyticsResponse {
  sessions_today: number;
  sessions_7d: SessionDailyCount[];
  avg_duration_mins: number;
  project_breakdown: SessionProjectBreakdown[];
  total_sessions: number;
}

// ── GET /api/sessions (paginated) ─────────────────────────────────────────

/** A single session item in the paginated sessions list. */
export interface SessionTimelineItem {
  id: string;
  project: string;
  command: string;
  status: string;
  trigger_type: string | null;
  message_count: number;
  tool_count: number;
  started_at: string;
  stopped_at: string | null;
  duration_display: string;
}

/** Paginated response from GET /api/sessions. */
export interface SessionListResponse {
  sessions: SessionTimelineItem[];
  total: number;
  page: number;
  limit: number;
}

// ── GET /api/sessions/[id]/events ─────────────────────────────────────────

/** A single session event from the session_events table. */
export interface SessionEventItem {
  id: string;
  session_id: string;
  event_type: string;
  direction: string | null;
  content: string | null;
  metadata: Record<string, unknown> | null;
  created_at: string;
}

/** Response from GET /api/sessions/[id]/events. */
export interface SessionEventsResponse {
  events: SessionEventItem[];
}

// ── GET /api/sessions/[id] ────────────────────────────────────────────────

/**
 * Detailed view of a single session from DB.
 * Source: Drizzle query on sessions table by ID.
 */
export interface SessionDetail {
  id: string;
  /** Derived from command: "CLI" | "Telegram" | command value */
  service: string;
  /** "active" when status is "running", otherwise as-is from DB */
  status: string;
  /** Message count from sessions table */
  messages: number;
  /** Tool execution count from sessions table */
  tools_executed: number;
  started_at: string;
  /** ISO timestamp from stopped_at column, or null if still running */
  ended_at: string | null;
  project: string;
  /** Trigger type from sessions table */
  trigger_type: string | null;
  /** Message count from sessions table */
  message_count: number;
  /** Tool count from sessions table */
  tool_count: number;
}

// ── Projects ──────────────────────────────────────────────────────────────

/** Project category values. */
export type ProjectCategory = "work" | "personal" | "open_source" | "archived";

/** Project status values. */
export type ProjectStatus = "active" | "paused" | "completed" | "archived";

/** A project entity backed by the projects DB table, enriched with counts. */
export interface ProjectEntity {
  id: string;
  code: string;
  name: string;
  category: ProjectCategory;
  status: ProjectStatus;
  description: string | null;
  content: string | null;
  path: string | null;
  obligation_count: number;
  active_obligation_count: number;
  session_count: number;
  last_activity: string | null;
  created_at: string;
  updated_at: string;
}

/** Response from GET /api/projects. */
export interface ProjectsListResponse {
  projects: ProjectEntity[];
}

/** Response from POST /api/projects/extract. */
export interface ProjectExtractionResponse {
  projects_updated: number;
  sources_scanned: string[];
}

/** Request body for POST /api/projects. */
export interface CreateProjectRequest {
  code: string;
  name: string;
  category?: ProjectCategory;
  path?: string;
}

/** Request body for PUT /api/projects/[code]. */
export interface UpdateProjectRequest {
  name?: string;
  category?: ProjectCategory;
  status?: ProjectStatus;
}

// ── Home Command Center (declutter-home-command-center) ───────────────────

/**
 * A single actionable item derived from obligations, messages, or automations.
 * Rendered in the ActionItems panel on the home dashboard.
 */
export interface ActionItem {
  id: string;
  severity: "error" | "warning";
  category: "obligation" | "message" | "automation";
  summary: string;
  /** Deep link to the relevant page (e.g. "/obligations", "/messages"). */
  link: string;
}

/**
 * A grouped summary of activity feed events for a single category.
 * Used by GroupedActivitySummaries to render collapsed/expanded sections.
 */
export interface CategorySummary {
  type: "message" | "obligation" | "session" | "system";
  count: number;
  summaryText: string;
  latestTimestamp: string;
  items: ActivityFeedEvent[];
}
