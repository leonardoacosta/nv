/** nv health — fleet + daemon + dashboard health check. */

import { exec } from "../lib/exec.js";
import { checkFleet, getChannels } from "../lib/fleet.js";
import {
  heading,
  subheading,
  green,
  red,
  yellow,
  gray,
  padRight,
} from "../lib/format.js";

async function getDaemonStatus(): Promise<{
  active: boolean;
  uptime: string;
}> {
  const { stdout, exitCode } = await exec("systemctl", [
    "--user",
    "is-active",
    "nova-ts.service",
  ]);
  if (exitCode !== 0 || stdout !== "active") {
    return { active: false, uptime: "" };
  }
  const prop = await exec("systemctl", [
    "--user",
    "show",
    "nova-ts.service",
    "--property=ActiveEnterTimestamp",
  ]);
  const match = prop.stdout.match(/=(.+)/);
  if (match?.[1]) {
    const started = new Date(match[1]);
    const elapsed = Date.now() - started.getTime();
    return { active: true, uptime: formatUptime(elapsed) };
  }
  return { active: true, uptime: "unknown" };
}

function formatUptime(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  if (days > 0) return `${days}d ${hours % 24}h`;
  if (hours > 0) return `${hours}h ${minutes % 60}m`;
  return `${minutes}m`;
}

async function getDashboardStatus(): Promise<{
  running: boolean;
  status: string;
}> {
  const { stdout, exitCode } = await exec("docker", [
    "inspect",
    "nv-dashboard-1",
    "--format",
    "{{.State.Status}}",
  ]);
  if (exitCode !== 0) {
    return { running: false, status: "not found" };
  }
  return { running: stdout === "running", status: stdout };
}

export async function health(): Promise<void> {
  heading("Nova Fleet Health");

  // Run all checks in parallel
  const [daemon, dashboard, results, channels] = await Promise.all([
    getDaemonStatus(),
    getDashboardStatus(),
    checkFleet(),
    getChannels(),
  ]);

  // Daemon
  console.log("");
  if (daemon.active) {
    console.log(
      `Daemon:     ${green("active")} (nova-ts.service)    uptime: ${daemon.uptime}`,
    );
  } else {
    console.log(`Daemon:     ${red("inactive")} (nova-ts.service)`);
  }

  // Dashboard
  if (dashboard.running) {
    console.log(`Dashboard:  ${green("running")} (nv-dashboard-1)`);
  } else {
    console.log(
      `Dashboard:  ${red(dashboard.status)} (nv-dashboard-1)`,
    );
  }

  // Fleet
  const healthyCount = results.filter((r) => r.healthy).length;
  const total = results.length;
  const fleetColor = healthyCount === total ? green : healthyCount > 0 ? yellow : red;

  subheading(
    `\nFleet Services (${fleetColor(`${healthyCount}/${total}`)} healthy):`,
  );
  for (const r of results) {
    const port = `:${r.port}`;
    const name = padRight(r.name, 16);
    if (r.healthy) {
      const ms = padRight(`${r.latencyMs}ms`, 6);
      console.log(`  ${port}  ${name} ${green("OK")}     ${gray(ms)}`);
    } else {
      const reason = r.error ? gray(`(${r.error})`) : "";
      console.log(`  ${port}  ${name} ${red("FAIL")}   ${reason}`);
    }
  }

  // Channels
  if (channels.length > 0) {
    subheading("\nChannels:");
    for (const ch of channels) {
      const name = padRight(ch.name, 12);
      const status =
        ch.status === "connected"
          ? green("connected")
          : ch.status === "disconnected"
            ? red(`disconnected ${gray("(stub)")}`)
            : yellow(ch.status);
      console.log(`  ${name}  ${status}`);
    }
  }

  console.log("");
}
