/** nv pim [status|all|N] — PIM activation from terminal. */

import { green, red, bold, gray } from "../lib/format.js";

const GRAPH_SVC = "http://127.0.0.1:4107";

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 5000);
  try {
    const res = await fetch(url, { ...init, signal: controller.signal });
    clearTimeout(timer);
    if (!res.ok) {
      throw new Error(`HTTP ${res.status}: ${await res.text()}`);
    }
    return (await res.json()) as T;
  } catch (err) {
    clearTimeout(timer);
    throw err;
  }
}

interface PimStatus {
  roles?: Array<{ number: number; name: string; active: boolean }>;
  [key: string]: unknown;
}

async function pimStatus(): Promise<void> {
  try {
    const data = await fetchJson<PimStatus>(`${GRAPH_SVC}/pim/status`);
    console.log(bold("PIM Status"));
    if (data.roles) {
      for (const role of data.roles) {
        const status = role.active ? green("active") : gray("inactive");
        console.log(`  ${role.number}. ${role.name}: ${status}`);
      }
    } else {
      console.log(JSON.stringify(data, null, 2));
    }
  } catch (err) {
    console.error(red(`Failed to get PIM status: ${err instanceof Error ? err.message : String(err)}`));
    process.exit(1);
  }
}

async function pimActivateAll(): Promise<void> {
  try {
    const data = await fetchJson<Record<string, unknown>>(
      `${GRAPH_SVC}/pim/activate-all`,
      { method: "POST" },
    );
    console.log(green("All PIM roles activated"));
    if (typeof data === "object" && data !== null) {
      console.log(gray(JSON.stringify(data)));
    }
  } catch (err) {
    console.error(red(`Failed to activate all: ${err instanceof Error ? err.message : String(err)}`));
    process.exit(1);
  }
}

async function pimActivate(roleNumber: number): Promise<void> {
  try {
    const data = await fetchJson<Record<string, unknown>>(
      `${GRAPH_SVC}/pim/activate`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ role_number: roleNumber }),
      },
    );
    console.log(green(`PIM role ${roleNumber} activated`));
    if (typeof data === "object" && data !== null) {
      console.log(gray(JSON.stringify(data)));
    }
  } catch (err) {
    console.error(red(`Failed to activate role ${roleNumber}: ${err instanceof Error ? err.message : String(err)}`));
    process.exit(1);
  }
}

export async function pim(arg?: string): Promise<void> {
  if (!arg || arg === "status") {
    return pimStatus();
  }
  if (arg === "all") {
    return pimActivateAll();
  }
  const num = parseInt(arg, 10);
  if (isNaN(num)) {
    console.error(`Invalid argument: ${arg}`);
    console.error("Usage: nv pim [status|all|<role_number>]");
    process.exit(1);
  }
  return pimActivate(num);
}
