import type { ServiceConfig } from "../config.js";
import { sshCloudPC } from "../ssh.js";
import { socksGet, socksPost, isSocksAvailable } from "../socks-client.js";
import { getMgmtToken, clearMgmtTokenCache } from "../token-cache.js";
import { sanitize } from "../utils.js";

const PIM_SCRIPT = "graph-pim.ps1";
const MGMT_BASE = "https://management.azure.com";

// ── Helpers ────────────────────────────────────────────────────────────

async function mgmtGet(config: ServiceConfig, path: string, timeoutMs: number = 30_000): Promise<string> {
  const url = `${MGMT_BASE}${path}`;
  const token = await getMgmtToken(config.cloudpcHost);
  try {
    return await socksGet(url, token, timeoutMs);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearMgmtTokenCache();
      const fresh = await getMgmtToken(config.cloudpcHost);
      return await socksGet(url, fresh, timeoutMs);
    }
    throw err;
  }
}

async function mgmtPost(config: ServiceConfig, path: string, body: unknown, timeoutMs: number = 30_000): Promise<string> {
  const url = `${MGMT_BASE}${path}`;
  const token = await getMgmtToken(config.cloudpcHost);
  try {
    return await socksPost(url, token, body, timeoutMs);
  } catch (err) {
    if (err instanceof Error && err.message.includes("401")) {
      clearMgmtTokenCache();
      const fresh = await getMgmtToken(config.cloudpcHost);
      return await socksPost(url, fresh, body, timeoutMs);
    }
    throw err;
  }
}

// ── Tool implementations ───────────────────────────────────────────────

/**
 * Get PIM-eligible Azure roles with activation status.
 * Shows both direct and group-based assignments.
 */
export async function pimStatus(config: ServiceConfig): Promise<string> {
  if (!(await isSocksAvailable())) {
    // Status queries 13 scopes sequentially — needs 45s+
    return sshCloudPC(
      config.cloudpcHost,
      config.cloudpcUserPath,
      PIM_SCRIPT,
      "-Action Status",
      60_000,
    );
  }

  // Query role eligibility schedule instances
  const path = `/providers/Microsoft.Authorization/roleEligibilityScheduleInstances?$filter=asTarget()&api-version=2020-10-01`;
  const raw = await mgmtGet(config, path, 45_000);
  const data = JSON.parse(raw) as {
    value?: Array<{
      properties?: {
        roleDefinitionId?: string;
        scope?: string;
        status?: string;
        startDateTime?: string;
        endDateTime?: string;
        expandedProperties?: {
          roleDefinition?: { displayName?: string };
          scope?: { displayName?: string };
        };
      };
    }>;
  };

  if (!data.value?.length) return "No PIM-eligible roles found.";

  return data.value
    .map((item, i) => {
      const props = item.properties;
      const roleName = props?.expandedProperties?.roleDefinition?.displayName ?? "Unknown Role";
      const scopeName = props?.expandedProperties?.scope?.displayName ?? props?.scope ?? "";
      const status = props?.status ?? "Unknown";
      return `${i + 1}. ${roleName}\n  Scope: ${scopeName}\n  Status: ${status}`;
    })
    .join("\n\n");
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
  if (!(await isSocksAvailable())) {
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

  // First get the eligible roles to find the one at roleNumber
  const eligiblePath = `/providers/Microsoft.Authorization/roleEligibilityScheduleInstances?$filter=asTarget()&api-version=2020-10-01`;
  const eligibleRaw = await mgmtGet(config, eligiblePath, 45_000);
  const eligible = JSON.parse(eligibleRaw) as { value?: Array<{ properties?: { roleDefinitionId?: string; scope?: string } }> };

  if (!eligible.value || roleNumber < 1 || roleNumber > eligible.value.length) {
    return `Invalid role number ${roleNumber}. Run pim_status to see available roles.`;
  }

  const role = eligible.value[roleNumber - 1]!;
  const roleDefId = role.properties?.roleDefinitionId ?? "";
  const scope = role.properties?.scope ?? "";

  // Create role assignment schedule request
  const requestId = crypto.randomUUID();
  const activatePath = `${scope}/providers/Microsoft.Authorization/roleAssignmentScheduleRequests/${requestId}?api-version=2020-10-01`;
  const body = {
    properties: {
      principalId: "", // Will be filled by the API from the token
      roleDefinitionId: roleDefId,
      requestType: "SelfActivate",
      justification: justification ?? "Activated via Nova",
      scheduleInfo: {
        startDateTime: new Date().toISOString(),
        expiration: {
          type: "AfterDuration",
          duration: "PT8H",
        },
      },
    },
  };

  const result = await mgmtPost(config, activatePath, body);
  return `Role activation requested.\n${result}`;
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
  if (!(await isSocksAvailable())) {
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

  // Get all eligible roles and activate each
  const eligiblePath = `/providers/Microsoft.Authorization/roleEligibilityScheduleInstances?$filter=asTarget()&api-version=2020-10-01`;
  const eligibleRaw = await mgmtGet(config, eligiblePath, 45_000);
  const eligible = JSON.parse(eligibleRaw) as { value?: Array<{ properties?: { roleDefinitionId?: string; scope?: string } }> };

  if (!eligible.value?.length) return "No PIM-eligible roles to activate.";

  const results: string[] = [];
  for (const role of eligible.value) {
    const roleDefId = role.properties?.roleDefinitionId ?? "";
    const scope = role.properties?.scope ?? "";
    const requestId = crypto.randomUUID();
    const path = `${scope}/providers/Microsoft.Authorization/roleAssignmentScheduleRequests/${requestId}?api-version=2020-10-01`;
    const body = {
      properties: {
        principalId: "",
        roleDefinitionId: roleDefId,
        requestType: "SelfActivate",
        justification: justification ?? "Activated via Nova",
        scheduleInfo: {
          startDateTime: new Date().toISOString(),
          expiration: { type: "AfterDuration", duration: "PT8H" },
        },
      },
    };

    try {
      await mgmtPost(config, path, body, 15_000);
      results.push(`Activated: ${roleDefId.split("/").pop()}`);
    } catch (err) {
      results.push(`Failed: ${roleDefId.split("/").pop()} — ${err instanceof Error ? err.message : "unknown error"}`);
    }
  }

  return results.join("\n");
}
