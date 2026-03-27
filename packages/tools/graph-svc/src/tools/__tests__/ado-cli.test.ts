/**
 * End-to-end tests for ADO CLI commands via SSH to CloudPC.
 *
 * These tests verify that the azure-devops CLI extension is installed and
 * functional on the CloudPC, and that our sshAdoCommand wrapper can execute
 * each category of ADO command and receive valid JSON.
 *
 * Requirements:
 *   - SSH access to "cloudpc" host (key-based auth, no password prompt)
 *   - Azure CLI logged in on CloudPC (`az login` completed)
 *   - azure-devops extension installed (`az extension add --name azure-devops`)
 *
 * Run with:
 *   npx tsx src/tools/__tests__/ado-cli.test.ts
 */

import { execFile } from "node:child_process";

const ADO_ORG = "https://dev.azure.com/brownandbrowninc";
const ADO_RESOURCE = "499b84ac-1321-427f-aa17-267ca6975798";
const SSH_HOST = process.env["CLOUDPC_HOST"] ?? "cloudpc";
const TEST_PROJECT = "LocalEdge";
const TEST_REPO = "agency-portal";

// ── Helpers ──────────────────────────────────────────────────────────

/** Run a PowerShell command on CloudPC via SSH with AAD token injection. */
function sshAdoRaw(adoCommand: string, timeoutMs = 45_000): Promise<string> {
  const ps = [
    `$token = az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
    `if (-not $token) { Write-Error 'Failed to acquire AAD token'; exit 1 }`,
    `$env:AZURE_DEVOPS_EXT_PAT = $token`,
    adoCommand,
  ].join("; ");

  const cmd = `powershell -NoProfile -ExecutionPolicy Bypass -Command "${ps}"`;

  return new Promise((resolve, reject) => {
    execFile(
      "ssh",
      ["-o", "ConnectTimeout=10", SSH_HOST, cmd],
      { timeout: timeoutMs },
      (error, stdout, stderr) => {
        if (error) {
          reject(new Error(`SSH failed: ${stderr?.trim() || error.message}`));
          return;
        }
        // Filter noise lines (same as production ssh.ts)
        const NOISE = ["WARNING:", "vulnerable", "upgraded", "security fix"];
        const filtered = (stdout ?? "")
          .split("\n")
          .filter((line) => !NOISE.some((p) => line.includes(p)))
          .join("\n")
          .trim();
        resolve(filtered);
      },
    );
  });
}

/** Parse JSON and return a typed result, or throw with context. */
function parseJSON(raw: string, label: string): unknown {
  try {
    return JSON.parse(raw);
  } catch {
    // Show first 500 chars for debugging
    throw new Error(
      `${label}: Failed to parse JSON.\nRaw output (first 500 chars):\n${raw.slice(0, 500)}`,
    );
  }
}

// ── Test runner ──────────────────────────────────────────────────────

interface TestResult {
  name: string;
  pass: boolean;
  durationMs: number;
  error?: string;
  itemCount?: number;
}

const results: TestResult[] = [];

async function test(
  name: string,
  fn: () => Promise<void>,
): Promise<void> {
  const start = Date.now();
  try {
    await fn();
    results.push({ name, pass: true, durationMs: Date.now() - start });
  } catch (err) {
    results.push({
      name,
      pass: false,
      durationMs: Date.now() - start,
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

// ── Tests ────────────────────────────────────────────────────────────

await test("SSH connectivity and token acquisition", async () => {
  const raw = await sshAdoRaw(
    `az account get-access-token --resource ${ADO_RESOURCE} --query accessToken -o tsv 2>$null`,
  );
  if (!raw || raw.length < 100) {
    throw new Error(`Token too short (${raw.length} chars) — likely failed`);
  }
});

await test("az devops project list (known working baseline)", async () => {
  const raw = await sshAdoRaw(
    `az devops project list --organization ${ADO_ORG} -o json 2>$null`,
  );
  const data = parseJSON(raw, "projects") as { value?: unknown[] };
  if (!Array.isArray(data.value) || data.value.length === 0) {
    throw new Error("Expected non-empty projects list");
  }
});

await test("az pipelines list (extension command)", async () => {
  const raw = await sshAdoRaw(
    `az pipelines list --organization ${ADO_ORG} --project '${TEST_PROJECT}' -o json 2>$null`,
  );
  const data = parseJSON(raw, "pipelines") as unknown[];
  if (!Array.isArray(data)) {
    throw new Error(`Expected array, got ${typeof data}`);
  }
  const current = results[results.length - 1];
  if (current) current.itemCount = data.length;
});

await test("az pipelines runs list (builds)", async () => {
  const raw = await sshAdoRaw(
    `az pipelines runs list --organization ${ADO_ORG} --project '${TEST_PROJECT}' --top 5 -o json 2>$null`,
  );
  const data = parseJSON(raw, "builds") as unknown[];
  if (!Array.isArray(data)) {
    throw new Error(`Expected array, got ${typeof data}`);
  }
  if (data.length === 0) {
    throw new Error("Expected at least one build run");
  }
  const current = results[results.length - 1];
  if (current) current.itemCount = data.length;
});

await test("az repos list", async () => {
  const raw = await sshAdoRaw(
    `az repos list --organization ${ADO_ORG} --project '${TEST_PROJECT}' -o json 2>$null`,
  );
  const data = parseJSON(raw, "repos") as unknown[];
  if (!Array.isArray(data)) {
    throw new Error(`Expected array, got ${typeof data}`);
  }
  const current = results[results.length - 1];
  if (current) current.itemCount = data.length;
});

await test("az devops invoke (commits)", async () => {
  const raw = await sshAdoRaw(
    `az devops invoke --organization ${ADO_ORG} --area git --resource commits --api-version 7.1 --route-parameters project='${TEST_PROJECT}' repositoryId='${TEST_REPO}' --query-parameters \\$top=5 -o json 2>$null`,
    60_000,
  );
  const data = parseJSON(raw, "commits") as { count?: number; value?: unknown[] };
  if (!Array.isArray(data.value)) {
    throw new Error(`Expected .value array, got ${typeof data.value}`);
  }
  const current = results[results.length - 1];
  if (current) current.itemCount = data.value.length;
});

await test("az pipelines show (pipeline definition)", async () => {
  // First get a pipeline ID from the list
  const listRaw = await sshAdoRaw(
    `az pipelines list --organization ${ADO_ORG} --project '${TEST_PROJECT}' --query '[0].id' -o tsv 2>$null`,
  );
  const pipelineId = parseInt(listRaw.trim(), 10);
  if (isNaN(pipelineId)) {
    throw new Error(`Could not get pipeline ID from list: "${listRaw}"`);
  }

  const raw = await sshAdoRaw(
    `az pipelines show --organization ${ADO_ORG} --id ${pipelineId} --project '${TEST_PROJECT}' -o json 2>$null`,
  );
  const data = parseJSON(raw, "pipeline-definition") as { id?: number; name?: string };
  if (data.id !== pipelineId) {
    throw new Error(`Expected pipeline id=${pipelineId}, got ${data.id}`);
  }
});

await test("az pipelines variable list", async () => {
  // Get first pipeline ID
  const listRaw = await sshAdoRaw(
    `az pipelines list --organization ${ADO_ORG} --project '${TEST_PROJECT}' --query '[0].id' -o tsv 2>$null`,
  );
  const pipelineId = parseInt(listRaw.trim(), 10);
  if (isNaN(pipelineId)) {
    throw new Error(`Could not get pipeline ID: "${listRaw}"`);
  }

  const raw = await sshAdoRaw(
    `az pipelines variable list --organization ${ADO_ORG} --pipeline-id ${pipelineId} --project '${TEST_PROJECT}' -o json 2>$null`,
  );
  // When no variables are configured, az CLI returns empty string (not even {}).
  // This is a known CLI quirk. Treat empty as valid "no variables" response.
  if (raw.trim() === "") {
    // No variables configured for this pipeline — acceptable.
    return;
  }
  const data = parseJSON(raw, "variables");
  if (typeof data !== "object" || data === null) {
    throw new Error(`Expected object, got ${typeof data}`);
  }
});

await test("az repos ref list (branches)", async () => {
  const raw = await sshAdoRaw(
    `az repos ref list --organization ${ADO_ORG} --repository '${TEST_REPO}' --project '${TEST_PROJECT}' --filter heads -o json 2>$null`,
  );
  const data = parseJSON(raw, "branches") as unknown[];
  if (!Array.isArray(data)) {
    throw new Error(`Expected array, got ${typeof data}`);
  }
  const current = results[results.length - 1];
  if (current) current.itemCount = data.length;
});

await test("az repos pr list (pull requests)", async () => {
  const raw = await sshAdoRaw(
    `az repos pr list --organization ${ADO_ORG} --project '${TEST_PROJECT}' -o json 2>$null`,
  );
  const data = parseJSON(raw, "pull-requests") as unknown[];
  if (!Array.isArray(data)) {
    throw new Error(`Expected array, got ${typeof data}`);
  }
  const current = results[results.length - 1];
  if (current) current.itemCount = data.length;
});

// ── Report ───────────────────────────────────────────────────────────

console.log("\n" + "=".repeat(70));
console.log("  ADO CLI Extension Test Results");
console.log("=".repeat(70));

let passed = 0;
let failed = 0;

for (const r of results) {
  const icon = r.pass ? "PASS" : "FAIL";
  const count = r.itemCount !== undefined ? ` (${r.itemCount} items)` : "";
  const time = `${(r.durationMs / 1000).toFixed(1)}s`;
  console.log(`  [${icon}] ${r.name} -- ${time}${count}`);
  if (r.error) {
    console.log(`         Error: ${r.error.split("\n")[0]}`);
  }
  if (r.pass) passed++;
  else failed++;
}

console.log("=".repeat(70));
console.log(`  Total: ${results.length} | Passed: ${passed} | Failed: ${failed}`);
console.log("=".repeat(70) + "\n");

process.exit(failed > 0 ? 1 : 0);
