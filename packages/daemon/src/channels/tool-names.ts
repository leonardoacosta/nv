/**
 * Humanize MCP tool names into user-friendly labels for streaming status.
 *
 * Patterns:
 *   mcp__nova-teams__teams_list_chats  -> "Checking Teams..."
 *   mcp__nova-calendar__calendar_list_events -> "Checking Calendar..."
 *   Read -> "Reading files..."
 *   unknown -> "Working..."
 */

const SERVER_NAMES: Record<string, string> = {
  "nova-teams": "Teams",
  "nova-calendar": "Calendar",
  "nova-discord": "Discord",
  "nova-mail": "Mail",
  "nova-contacts": "Contacts",
  "nova-ado": "Azure DevOps",
  "nova-graph": "Microsoft Graph",
  "nova-memory": "Memory",
  "nova-meta": "Meta",
};

const ACTION_VERBS: Record<string, string> = {
  list_: "Checking",
  get_: "Reading",
  search_: "Searching",
  send_: "Sending",
  create_: "Creating",
  update_: "Updating",
  delete_: "Deleting",
};

const BUILTIN_TOOLS: Record<string, string> = {
  Read: "Reading files...",
  Write: "Writing files...",
  Bash: "Running command...",
  Glob: "Searching files...",
  Grep: "Searching files...",
  WebSearch: "Searching the web...",
  WebFetch: "Fetching page...",
};

export function humanizeToolName(rawName: string): string {
  // Check built-in tools first
  const builtin = BUILTIN_TOOLS[rawName];
  if (builtin) return builtin;

  // Parse mcp__<server>__<tool> pattern
  const mcpMatch = rawName.match(/^mcp__([^_]+(?:-[^_]+)*)__(.+)$/);
  if (!mcpMatch) return "Working...";

  const serverKey = mcpMatch[1]!;
  const toolPart = mcpMatch[2]!;

  const serviceName = SERVER_NAMES[serverKey];
  if (!serviceName) return "Working...";

  // Find matching action verb from the tool name
  for (const [prefix, verb] of Object.entries(ACTION_VERBS)) {
    if (toolPart.includes(prefix)) {
      return `${verb} ${serviceName}...`;
    }
  }

  // Known service but unknown action
  return `Using ${serviceName}...`;
}
