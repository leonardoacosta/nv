/** nv restart [service] — restart systemd services. */

import { exec } from "../lib/exec.js";
import { green, red } from "../lib/format.js";

const SERVICE_MAP: Record<string, string[]> = {
  daemon: ["nova-ts.service"],
  fleet: ["nova-tools.target"],
  router: ["nova-tool-router.service"],
  memory: ["nova-memory-svc.service"],
  messages: ["nova-messages-svc.service"],
  channels: ["nova-channels-svc.service"],
  discord: ["nova-discord-svc.service"],
  teams: ["nova-teams-svc.service"],
  schedule: ["nova-schedule-svc.service"],
  graph: ["nova-graph-svc.service"],
  meta: ["nova-meta-svc.service"],
  azure: ["nova-azure-svc.service"],
  all: ["nova-ts.service", "nova-tools.target"],
};

export async function restart(service?: string): Promise<void> {
  const target = service ?? "daemon";
  const units = SERVICE_MAP[target];

  if (!units) {
    console.error(`Unknown service: ${target}`);
    console.error(`Available: ${Object.keys(SERVICE_MAP).join(", ")}`);
    process.exit(1);
  }

  for (const unit of units) {
    console.log(`Restarting ${unit}...`);
    const { exitCode, stderr } = await exec(
      "systemctl",
      ["--user", "restart", unit],
      10000,
    );
    if (exitCode === 0) {
      console.log(`  ${green("OK")} ${unit} restarted`);
    } else {
      console.log(`  ${red("FAIL")} ${unit}: ${stderr}`);
    }
  }

  // Also restart dashboard container if 'all'
  if (target === "all") {
    console.log("Restarting nv-dashboard-1...");
    const { exitCode, stderr } = await exec(
      "docker",
      ["restart", "nv-dashboard-1"],
      15000,
    );
    if (exitCode === 0) {
      console.log(`  ${green("OK")} nv-dashboard-1 restarted`);
    } else {
      console.log(`  ${red("FAIL")} nv-dashboard-1: ${stderr}`);
    }
  }
}
