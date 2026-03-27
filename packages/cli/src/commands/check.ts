/** nv check — full connectivity check (fleet + postgres + SSH + Doppler + env). */

import { exec } from "../lib/exec.js";
import { checkFleet } from "../lib/fleet.js";
import {
  subheading,
  heading,
  check,
  cross,
  circle,
  padRight,
  gray,
} from "../lib/format.js";

interface EnvVar {
  name: string;
  required: boolean;
}

const EXPECTED_ENV_VARS: EnvVar[] = [
  { name: "DATABASE_URL", required: true },
  { name: "TELEGRAM_BOT_TOKEN", required: true },
  { name: "DISCORD_BOT_TOKEN", required: true },
  { name: "VERCEL_GATEWAY_KEY", required: true },
  { name: "OPENAI_API_KEY", required: false },
];

async function checkPostgres(): Promise<{
  connected: boolean;
  tableCount: number | null;
  error?: string;
}> {
  // Try to get DATABASE_URL from Doppler
  const { stdout: dbUrl, exitCode } = await exec(
    "doppler",
    ["secrets", "get", "DATABASE_URL", "--plain", "--project", "nova", "--config", "prd"],
    5000,
  );
  if (exitCode !== 0 || !dbUrl) {
    return { connected: false, tableCount: null, error: "no DATABASE_URL" };
  }

  // Test connection and count tables
  const { stdout, exitCode: pgExit } = await exec(
    "psql",
    [
      dbUrl,
      "-t",
      "-A",
      "-c",
      "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public'",
    ],
    5000,
  );
  if (pgExit !== 0) {
    return { connected: false, tableCount: null, error: "connection failed" };
  }
  const count = parseInt(stdout, 10);
  return { connected: true, tableCount: isNaN(count) ? null : count };
}

async function checkSSH(): Promise<{
  reachable: boolean;
  latencyMs: number | null;
}> {
  const start = performance.now();
  const { exitCode } = await exec(
    "ssh",
    ["-o", "ConnectTimeout=3", "-o", "BatchMode=yes", "cloudpc", "echo", "ok"],
    5000,
  );
  const latencyMs = Math.round(performance.now() - start);
  return { reachable: exitCode === 0, latencyMs: exitCode === 0 ? latencyMs : null };
}

async function checkDoppler(): Promise<{
  configured: boolean;
  error?: string;
}> {
  const { exitCode } = await exec(
    "doppler",
    ["secrets", "--project", "nova", "--config", "prd", "--only-names"],
    5000,
  );
  return { configured: exitCode === 0 };
}

async function checkEnvVars(): Promise<
  { name: string; set: boolean; required: boolean }[]
> {
  // Try Doppler first for a bulk check
  const { stdout, exitCode } = await exec(
    "doppler",
    ["secrets", "--project", "nova", "--config", "prd", "--only-names"],
    5000,
  );
  const dopplerKeys = exitCode === 0 ? new Set(stdout.split("\n").filter(Boolean)) : new Set<string>();

  return EXPECTED_ENV_VARS.map((v) => ({
    name: v.name,
    set: dopplerKeys.has(v.name) || !!process.env[v.name],
    required: v.required,
  }));
}

export async function checkCmd(): Promise<void> {
  heading("Nova Connectivity Check");

  // Run all checks in parallel
  const [fleetResults, pg, ssh, doppler, envVars] = await Promise.all([
    checkFleet(),
    checkPostgres(),
    checkSSH(),
    checkDoppler(),
    checkEnvVars(),
  ]);

  // Fleet
  subheading("\nFleet Services:");
  for (const r of fleetResults) {
    const name = padRight(r.name, 16);
    const port = padRight(`:${r.port}`, 8);
    if (r.healthy) {
      console.log(check(`${name} ${port} ${gray(`${r.latencyMs}ms`)}`));
    } else {
      console.log(cross(`${name} ${port} ${gray(r.error ?? "unreachable")}`));
    }
  }

  // Connectivity
  subheading("\nConnectivity:");

  if (pg.connected) {
    const tables = pg.tableCount !== null ? `${pg.tableCount} tables` : "connected";
    console.log(check(`${padRight("postgres", 16)} :5436    connected ${gray(`(${tables})`)}`));
  } else {
    console.log(cross(`${padRight("postgres", 16)} :5436    ${gray(pg.error ?? "unreachable")}`));
  }

  if (ssh.reachable) {
    console.log(
      check(`${padRight("cloudpc", 16)} SSH      reachable ${gray(`(${ssh.latencyMs}ms)`)}`),
    );
  } else {
    console.log(cross(`${padRight("cloudpc", 16)} SSH      unreachable`));
  }

  if (doppler.configured) {
    console.log(check(`${padRight("doppler", 16)} nova/prd configured`));
  } else {
    console.log(cross(`${padRight("doppler", 16)} nova/prd not configured`));
  }

  // Environment
  subheading("\nEnvironment:");
  for (const v of envVars) {
    const name = padRight(v.name, 20);
    if (v.set) {
      console.log(check(`${name} set`));
    } else if (v.required) {
      console.log(cross(`${name} not set`));
    } else {
      console.log(circle(`${name} not set ${gray("(optional)")}`));
    }
  }

  console.log("");
}
