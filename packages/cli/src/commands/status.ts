/** nv status — quick one-line status. */

import { exec } from "../lib/exec.js";
import { checkFleet, getChannels } from "../lib/fleet.js";
import { green, red, yellow } from "../lib/format.js";

export async function status(): Promise<void> {
  const [daemonResult, dashboardResult, results, channels] = await Promise.all([
    exec("systemctl", ["--user", "is-active", "nova-ts.service"]),
    exec("docker", [
      "inspect",
      "nv-dashboard-1",
      "--format",
      "{{.State.Status}}",
    ]),
    checkFleet(),
    getChannels(),
  ]);

  const daemonActive = daemonResult.stdout === "active";
  const dashboardRunning = dashboardResult.stdout === "running";
  const healthyCount = results.filter((r) => r.healthy).length;
  const totalCount = results.length;
  const connectedChannels = channels.filter(
    (c) => c.status === "connected",
  ).length;
  const totalChannels = channels.length || 5; // fallback to 5 known channels

  const daemonStr = daemonActive ? green("active") : red("inactive");
  const fleetColor =
    healthyCount === totalCount
      ? green
      : healthyCount > 0
        ? yellow
        : red;
  const fleetStr = fleetColor(`${healthyCount}/${totalCount}`);
  const dashboardStr = dashboardRunning ? green("running") : red("stopped");
  const channelStr =
    connectedChannels > 0
      ? green(`${connectedChannels}/${totalChannels}`)
      : red(`0/${totalChannels}`);

  console.log(
    `Nova: daemon=${daemonStr} fleet=${fleetStr} dashboard=${dashboardStr} channels=${channelStr}`,
  );
}
