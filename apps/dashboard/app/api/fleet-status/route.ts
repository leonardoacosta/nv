import { NextResponse } from "next/server";
import type {
  FleetHealthResponse,
  FleetServiceStatus,
  ChannelStatus,
} from "@/types/api";

/**
 * Static fleet service registry.
 *
 * These services run on the host (127.0.0.1) and are NOT reachable from the
 * Docker container where the dashboard runs. We show their configured URLs
 * with "unknown" status rather than faking "Connected".
 */
const FLEET_SERVICES: Omit<FleetServiceStatus, "status" | "latency_ms">[] = [
  { name: "tool-router", url: "http://127.0.0.1:4100", port: 4100, tools: [] },
  { name: "memory-svc", url: "http://127.0.0.1:4101", port: 4101, tools: ["read_memory", "write_memory", "search_memory"] },
  { name: "messages-svc", url: "http://127.0.0.1:4102", port: 4102, tools: ["get_recent_messages", "search_messages"] },
  { name: "channels-svc", url: "http://127.0.0.1:4103", port: 4103, tools: ["list_channels", "send_to_channel"] },
  { name: "discord-svc", url: "http://127.0.0.1:4104", port: 4104, tools: ["discord_list_guilds", "discord_list_channels", "discord_read_messages"] },
  { name: "teams-svc", url: "http://127.0.0.1:4105", port: 4105, tools: ["teams_list_chats", "teams_read_chat", "teams_messages", "teams_channels", "teams_presence", "teams_send"] },
  { name: "schedule-svc", url: "http://127.0.0.1:4106", port: 4106, tools: ["set_reminder", "cancel_reminder", "list_reminders", "add_schedule", "modify_schedule", "remove_schedule", "list_schedules", "start_session", "stop_session"] },
  { name: "graph-svc", url: "http://127.0.0.1:4107", port: 4107, tools: ["calendar_today", "calendar_upcoming", "calendar_next", "ado_projects", "ado_pipelines", "ado_builds", "outlook_inbox", "outlook_read", "outlook_search", "outlook_folders", "outlook_sent", "outlook_folder"] },
  { name: "meta-svc", url: "http://127.0.0.1:4108", port: 4108, tools: ["check_services", "self_assessment_run", "update_soul"] },
  { name: "azure-svc", url: "http://127.0.0.1:4109", port: 4109, tools: ["azure_cli"] },
];

/**
 * Known channels derived from nv.toml configuration.
 * These are the channels Nova is configured to use.
 */
const KNOWN_CHANNELS: ChannelStatus[] = [
  { name: "Telegram", status: "configured", direction: "bidirectional" },
  { name: "Discord", status: "configured", direction: "bidirectional" },
  { name: "Microsoft Teams", status: "configured", direction: "bidirectional" },
];

export async function GET() {
  const services: FleetServiceStatus[] = FLEET_SERVICES.map((svc) => ({
    ...svc,
    status: "unknown" as const,
    latency_ms: null,
  }));

  const response: FleetHealthResponse = {
    fleet: {
      status: "unknown",
      services,
      healthy_count: 0,
      total_count: services.length,
    },
    channels: KNOWN_CHANNELS,
  };

  return NextResponse.json(response);
}
