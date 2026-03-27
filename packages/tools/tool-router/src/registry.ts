/** Tool-to-service registry for the Nova tool fleet. */

export interface ServiceEntry {
  serviceUrl: string;
  serviceName: string;
}

/** All 9 downstream services and their tool mappings. */
const SERVICES = {
  "memory-svc": { url: "http://127.0.0.1:4101", tools: ["read_memory", "write_memory", "search_memory"] },
  "messages-svc": { url: "http://127.0.0.1:4102", tools: ["get_recent_messages", "search_messages"] },
  "channels-svc": { url: "http://127.0.0.1:4103", tools: ["list_channels", "send_to_channel"] },
  "discord-svc": { url: "http://127.0.0.1:4104", tools: ["discord_list_guilds", "discord_list_channels", "discord_read_messages"] },
  "teams-svc": { url: "http://127.0.0.1:4105", tools: ["teams_list_chats", "teams_read_chat", "teams_messages", "teams_channels", "teams_presence", "teams_send"] },
  "schedule-svc": { url: "http://127.0.0.1:4106", tools: ["set_reminder", "cancel_reminder", "list_reminders", "add_schedule", "modify_schedule", "remove_schedule", "list_schedules", "start_session", "stop_session"] },
  "graph-svc": { url: "http://127.0.0.1:4107", tools: ["calendar_today", "calendar_upcoming", "calendar_next", "ado_projects", "ado_pipelines", "ado_builds", "ado_work_items", "ado_repos", "ado_pull_requests", "ado_build_logs", "outlook_inbox", "outlook_read", "outlook_search", "outlook_folders", "outlook_sent", "outlook_folder", "outlook_flag", "outlook_move", "outlook_unread", "pim_status", "pim_activate", "pim_activate_all"] },
  "meta-svc": { url: "http://127.0.0.1:4108", tools: ["check_services", "self_assessment_run", "update_soul", "typecheck_project", "build_project"] },
  "azure-svc": { url: "http://127.0.0.1:4109", tools: ["azure_cli"] },
} as const satisfies Record<string, { url: string; tools: readonly string[] }>;

/** Flat map: tool name -> { serviceUrl, serviceName } */
const TOOL_MAP: ReadonlyMap<string, ServiceEntry> = (() => {
  const map = new Map<string, ServiceEntry>();
  for (const [serviceName, def] of Object.entries(SERVICES)) {
    for (const tool of def.tools) {
      map.set(tool, { serviceUrl: def.url, serviceName });
    }
  }
  return map;
})();

/** Unique service list with their base URLs. */
export interface ServiceInfo {
  serviceName: string;
  serviceUrl: string;
  tools: readonly string[];
}

/**
 * Look up which service handles a given tool name.
 * Returns undefined if the tool is not registered.
 */
export function getServiceForTool(name: string): ServiceEntry | undefined {
  return TOOL_MAP.get(name);
}

/** Return every registered service with its URL and tools list. */
export function getAllServices(): ServiceInfo[] {
  return Object.entries(SERVICES).map(([serviceName, def]) => ({
    serviceName,
    serviceUrl: def.url,
    tools: def.tools,
  }));
}

/** Return the full flat tool -> service mapping (for /registry). */
export function getFullRegistry(): Record<string, ServiceEntry> {
  const obj: Record<string, ServiceEntry> = {};
  for (const [tool, entry] of TOOL_MAP) {
    obj[tool] = entry;
  }
  return obj;
}
