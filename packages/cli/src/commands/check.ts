/** nv check -- deep validation of fleet services, connectivity, tokens, and env. */

import { exec } from "../lib/exec.js";
import { checkFleetDeep, type DeepCheckResult } from "../lib/fleet.js";
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

// ---------------------------------------------------------------------------
// Connectivity checks
// ---------------------------------------------------------------------------

interface PgResult {
  connected: boolean;
  tableCount: number | null;
  rowSummary: string | null;
  error?: string;
}

async function checkPostgres(): Promise<PgResult> {
  const { stdout: dbUrl, exitCode } = await exec(
    "doppler",
    ["secrets", "get", "DATABASE_URL", "--plain", "--project", "nova", "--config", "prd"],
    5000,
  );
  if (exitCode !== 0 || !dbUrl) {
    return { connected: false, tableCount: null, rowSummary: null, error: "no DATABASE_URL" };
  }

  // Table count
  const { stdout: tableOut, exitCode: tblExit } = await exec(
    "psql",
    [
      dbUrl, "-t", "-A", "-c",
      "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public'",
    ],
    5000,
  );
  if (tblExit !== 0) {
    return { connected: false, tableCount: null, rowSummary: null, error: "connection failed" };
  }
  const tableCount = parseInt(tableOut, 10);

  // Aggregate row count across key tables
  const { stdout: rowOut } = await exec(
    "psql",
    [
      dbUrl, "-t", "-A", "-c",
      [
        "SELECT coalesce(sum(c), 0) FROM (",
        "  SELECT count(*) AS c FROM messages",
        "  UNION ALL SELECT count(*) FROM memory",
        "  UNION ALL SELECT count(*) FROM obligations",
        ") sub",
      ].join(" "),
    ],
    5000,
  );
  const totalRows = parseInt(rowOut, 10);
  const rowSummary = isNaN(totalRows) ? null : totalRows.toLocaleString();

  return {
    connected: true,
    tableCount: isNaN(tableCount) ? null : tableCount,
    rowSummary,
  };
}

interface SshResult {
  reachable: boolean;
  latencyMs: number | null;
}

async function checkSSH(): Promise<SshResult> {
  const start = performance.now();
  const { exitCode } = await exec(
    "ssh",
    ["-o", "ConnectTimeout=3", "-o", "BatchMode=yes", "cloudpc", "echo", "ok"],
    5000,
  );
  const latencyMs = Math.round(performance.now() - start);
  return { reachable: exitCode === 0, latencyMs: exitCode === 0 ? latencyMs : null };
}

async function checkDoppler(): Promise<{ configured: boolean }> {
  const { exitCode } = await exec(
    "doppler",
    ["secrets", "--project", "nova", "--config", "prd", "--only-names"],
    5000,
  );
  return { configured: exitCode === 0 };
}

// ---------------------------------------------------------------------------
// Graph token checks (SSH to CloudPC, read token files)
// ---------------------------------------------------------------------------

interface TokenResult {
  name: string;
  file: string;
  valid: boolean;
  expiresIn: string | null;
  error?: string;
}

async function checkGraphTokens(): Promise<TokenResult[]> {
  const tokens = [
    { name: "O365", file: ".graph-token.json" },
    { name: "BBAdmin", file: ".graph-pim-token.json" },
  ];

  const results = await Promise.allSettled(
    tokens.map(async (t): Promise<TokenResult> => {
      // CloudPC is Windows — read token file via PowerShell
      // Use double quotes so $env:USERPROFILE expands, escape inner quotes for SSH
      const psCmd = `$f = Join-Path $env:USERPROFILE '${t.file}'; if (Test-Path $f) { Get-Content $f -Raw } else { 'MISSING' }`;
      const { stdout, exitCode } = await exec(
        "ssh",
        ["-o", "ConnectTimeout=5", "cloudpc", `powershell -NoProfile -Command "${psCmd}"`],
        15_000,
      );
      if (exitCode !== 0 || !stdout || stdout.trim() === "MISSING") {
        return { name: t.name, file: t.file, valid: false, expiresIn: null, error: "file not found" };
      }
      try {
        const parsed = JSON.parse(stdout) as { expires_on?: number; expiresOn?: string; expiry?: string; access_token?: string };
        if (!parsed.access_token) {
          return { name: t.name, file: t.file, valid: false, expiresIn: null, error: "no access_token" };
        }
        // Determine expiry — token files use different field names
        let expiresAt: number | null = null;
        if (typeof parsed.expires_on === "number") {
          expiresAt = parsed.expires_on * 1000; // Unix seconds -> ms
        } else if (typeof parsed.expiry === "string") {
          expiresAt = new Date(parsed.expiry).getTime(); // ISO datetime string
        } else if (typeof parsed.expiresOn === "string") {
          expiresAt = new Date(parsed.expiresOn).getTime();
        }
        if (expiresAt === null || isNaN(expiresAt)) {
          return { name: t.name, file: t.file, valid: true, expiresIn: "unknown expiry" };
        }
        const diffMs = expiresAt - Date.now();
        if (diffMs <= 0) {
          return { name: t.name, file: t.file, valid: false, expiresIn: null, error: "expired" };
        }
        return { name: t.name, file: t.file, valid: true, expiresIn: formatDuration(diffMs) };
      } catch {
        return { name: t.name, file: t.file, valid: false, expiresIn: null, error: "invalid JSON" };
      }
    }),
  );

  return results.map((r, i) => {
    if (r.status === "fulfilled") return r.value;
    return {
      name: tokens[i]!.name,
      file: tokens[i]!.file,
      valid: false,
      expiresIn: null,
      error: r.reason instanceof Error ? r.reason.message : String(r.reason),
    };
  });
}

function formatDuration(ms: number): string {
  const totalMin = Math.floor(ms / 60_000);
  if (totalMin < 60) return `${totalMin}m`;
  const hours = Math.floor(totalMin / 60);
  const mins = totalMin % 60;
  return `${hours}h ${mins}m`;
}

// ---------------------------------------------------------------------------
// Environment check
// ---------------------------------------------------------------------------

async function checkEnvVars(): Promise<{ name: string; set: boolean; required: boolean }[]> {
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

// ---------------------------------------------------------------------------
// Main command
// ---------------------------------------------------------------------------

function deepIcon(r: DeepCheckResult): string {
  switch (r.status) {
    case "ok":
      return check("");
    case "empty":
      return circle("");
    case "error":
      return cross("");
  }
}

function formatDeep(r: DeepCheckResult): string {
  const label = padRight(r.label, 18);
  const port = padRight(`:${r.port}`, 8);
  const ms = gray(`${r.latencyMs}ms`);
  // deepIcon already has leading spaces; trim trailing space for alignment
  return `${deepIcon(r).trimEnd()} ${label} ${port} ${r.detail}  ${ms}`;
}

export async function checkCmd(): Promise<void> {
  heading("Nova Deep Check");

  // Fire all checks in parallel
  const [deepResults, pg, ssh, doppler, tokens, envVars] = await Promise.all([
    checkFleetDeep(),
    checkPostgres(),
    checkSSH(),
    checkDoppler(),
    checkGraphTokens(),
    checkEnvVars(),
  ]);

  // --- Fleet Services ---
  subheading("\nFleet Services:");
  for (const r of deepResults) {
    console.log(formatDeep(r));
  }

  // --- Connectivity ---
  subheading("\nConnectivity:");

  if (pg.connected) {
    const tables = pg.tableCount !== null ? `${pg.tableCount} tables` : "";
    const rows = pg.rowSummary ? `, ${pg.rowSummary} rows` : "";
    console.log(check(`${padRight("postgres", 18)} :5436    connected ${gray(`(${tables}${rows})`)}`));
  } else {
    console.log(cross(`${padRight("postgres", 18)} :5436    ${gray(pg.error ?? "unreachable")}`));
  }

  if (ssh.reachable) {
    console.log(check(`${padRight("cloudpc", 18)} SSH      reachable ${gray(`${ssh.latencyMs}ms`)}`));
  } else {
    console.log(cross(`${padRight("cloudpc", 18)} SSH      unreachable`));
  }

  if (doppler.configured) {
    console.log(check(`${padRight("doppler", 18)} nova/prd configured`));
  } else {
    console.log(cross(`${padRight("doppler", 18)} nova/prd not configured`));
  }

  // --- Graph Tokens ---
  subheading("\nGraph Tokens:");
  for (const t of tokens) {
    const label = padRight(`${t.name} (${t.file})`, 34);
    if (t.valid) {
      const expiry = t.expiresIn ? `, expires in ${t.expiresIn}` : "";
      console.log(check(`${label} valid${expiry}`));
    } else {
      console.log(cross(`${label} ${gray(t.error ?? "invalid")}`));
    }
  }

  // --- Environment ---
  subheading("\nEnvironment:");
  for (const v of envVars) {
    const name = padRight(v.name, 22);
    if (v.set) {
      console.log(check(`${name} set`));
    } else if (v.required) {
      console.log(cross(`${name} not set`));
    } else {
      console.log(circle(`${name} not set ${gray("(optional)")}`));
    }
  }

  // --- Summary ---
  const serviceOk = deepResults.filter((r) => r.status === "ok").length;
  const serviceTotal = deepResults.length;
  const tokensValid = tokens.filter((t) => t.valid).length;
  const tokensTotal = tokens.length;
  console.log(
    `\nSummary: ${serviceOk}/${serviceTotal} services healthy, ${tokensValid}/${tokensTotal} tokens valid`,
  );
}
