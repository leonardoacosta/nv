import type { ServiceConfig } from "../config.js";
import { sshCloudPC } from "../ssh.js";
import { sanitize } from "../utils.js";

const PIM_SCRIPT = "graph-pim.ps1";

/**
 * Get PIM-eligible Azure roles with activation status.
 * Shows both direct and group-based assignments.
 */
export async function pimStatus(config: ServiceConfig): Promise<string> {
  // Status queries 13 scopes sequentially — needs 45s+
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    PIM_SCRIPT,
    "-Action Status",
    60_000,
  );
}

/**
 * Activate a specific PIM role by number.
 * @param roleNumber The role number from pimStatus output
 * @param justification Optional justification for the activation
 */
export async function pimActivate(
  config: ServiceConfig,
  roleNumber: number,
  justification?: string,
): Promise<string> {
  let args = `-Action Activate -RoleNumber ${roleNumber}`;
  if (justification) {
    args += ` -Justification '${sanitize(justification)}'`;
  }
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    PIM_SCRIPT,
    args,
  );
}

/**
 * Activate all PIM-eligible Azure roles at once.
 * Uses 120s timeout as this queries multiple subscriptions.
 * @param justification Optional justification for the activation
 */
export async function pimActivateAll(
  config: ServiceConfig,
  justification?: string,
): Promise<string> {
  let args = "-Action ActivateAll";
  if (justification) {
    args += ` -Justification '${sanitize(justification)}'`;
  }
  return sshCloudPC(
    config.cloudpcHost,
    config.cloudpcUserPath,
    PIM_SCRIPT,
    args,
    120_000,
  );
}
