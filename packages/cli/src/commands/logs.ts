/** nv logs [service] — tail journalctl logs. */

import { execPassthrough } from "../lib/exec.js";

const SERVICE_MAP: Record<string, string> = {
  daemon: "nova-ts.service",
  fleet: "nova-tools.target",
  router: "nova-tool-router.service",
  memory: "nova-memory-svc.service",
  messages: "nova-messages-svc.service",
  channels: "nova-channels-svc.service",
  discord: "nova-discord-svc.service",
  teams: "nova-teams-svc.service",
  schedule: "nova-schedule-svc.service",
  graph: "nova-graph-svc.service",
  meta: "nova-meta-svc.service",
  azure: "nova-azure-svc.service",
};

export function logs(service?: string): void {
  const target = service ?? "daemon";
  const unit = SERVICE_MAP[target];

  if (!unit) {
    console.error(`Unknown service: ${target}`);
    console.error(`Available: ${Object.keys(SERVICE_MAP).join(", ")}`);
    process.exit(1);
  }

  execPassthrough("journalctl", ["--user", "-u", unit, "-f", "--no-pager"]);
}
