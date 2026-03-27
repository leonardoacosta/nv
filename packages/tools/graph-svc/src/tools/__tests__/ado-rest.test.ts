/**
 * Integration tests for the Azure DevOps REST API approach.
 *
 * These are LIVE tests that:
 *   1. SSH to the CloudPC to acquire an AAD token
 *   2. Make real REST API calls to Azure DevOps
 *
 * Run with: npx tsx src/tools/__tests__/ado-rest.test.ts
 *
 * Prerequisites:
 *   - SSH access to the CloudPC host (default: "cloudpc")
 *   - az CLI logged in on the CloudPC
 */

import { getAdoToken, adoRest, clearTokenCache, ADO_ORG } from "../ado-rest.js";

const HOST = process.env["CLOUDPC_HOST"] ?? "cloudpc";

// ── Helpers ─────────────────────────────────────────────────────────────

let passed = 0;
let failed = 0;
const errors: string[] = [];

async function test(name: string, fn: () => Promise<void>): Promise<void> {
  try {
    await fn();
    console.log(`  PASS  ${name}`);
    passed++;
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.log(`  FAIL  ${name}`);
    console.log(`        ${msg}`);
    failed++;
    errors.push(`${name}: ${msg}`);
  }
}

function assert(condition: boolean, message: string): void {
  if (!condition) throw new Error(`Assertion failed: ${message}`);
}

// ── Tests ───────────────────────────────────────────────────────────────

async function main() {
  console.log("\n=== Azure DevOps REST API Integration Tests ===\n");

  // ── 1. Token acquisition ──────────────────────────────────────────────
  await test("acquires AAD token via SSH", async () => {
    clearTokenCache();
    const token = await getAdoToken(HOST);
    assert(typeof token === "string", "token should be a string");
    assert(token.length > 50, `token should be long, got ${token.length} chars`);
    // AAD tokens are JWTs -- they have 3 dot-separated parts
    const parts = token.split(".");
    assert(parts.length === 3, `expected JWT with 3 parts, got ${parts.length}`);
    console.log(`        Token acquired: ${token.slice(0, 20)}...`);
  });

  await test("returns cached token on second call", async () => {
    const t1 = await getAdoToken(HOST);
    const t2 = await getAdoToken(HOST);
    assert(t1 === t2, "second call should return same cached token");
  });

  // ── 2. List projects ──────────────────────────────────────────────────
  let firstProject: string | undefined;

  await test("GET _apis/projects -- lists projects", async () => {
    const raw = await adoRest(HOST, "_apis/projects");
    const data = JSON.parse(raw);
    assert(data.count > 0, `expected at least 1 project, got ${data.count}`);
    assert(Array.isArray(data.value), "value should be an array");
    firstProject = data.value[0].name;
    console.log(`        Found ${data.count} projects, first: "${firstProject}"`);
  });

  // ── 3. List pipelines ─────────────────────────────────────────────────
  await test("GET {project}/_apis/pipelines -- lists pipelines", async () => {
    if (!firstProject) throw new Error("skipped -- no project available");
    const raw = await adoRest(HOST, `${encodeURIComponent(firstProject)}/_apis/pipelines`);
    const data = JSON.parse(raw);
    assert(data.count !== undefined, "response should have count");
    console.log(`        Found ${data.count} pipelines in "${firstProject}"`);
  });

  // ── 4. List builds ────────────────────────────────────────────────────
  await test("GET {project}/_apis/build/builds -- lists builds", async () => {
    if (!firstProject) throw new Error("skipped -- no project available");
    const raw = await adoRest(HOST, `${encodeURIComponent(firstProject)}/_apis/build/builds`, {
      query: { $top: 5 },
    });
    const data = JSON.parse(raw);
    assert(data.count !== undefined, "response should have count");
    console.log(`        Found ${data.count} builds in "${firstProject}" (top 5)`);
    if (data.value && data.value.length > 0) {
      const b = data.value[0];
      console.log(`        Latest build: #${b.id} ${b.status} (${b.result ?? "in progress"})`);
    }
  });

  // ── 5. List repositories ──────────────────────────────────────────────
  let firstRepo: string | undefined;

  await test("GET {project}/_apis/git/repositories -- lists repos", async () => {
    if (!firstProject) throw new Error("skipped -- no project available");
    const raw = await adoRest(
      HOST,
      `${encodeURIComponent(firstProject)}/_apis/git/repositories`,
    );
    const data = JSON.parse(raw);
    assert(Array.isArray(data.value), "value should be an array");
    console.log(`        Found ${data.value.length} repositories`);
    if (data.value.length > 0) {
      firstRepo = data.value[0].name;
      console.log(`        First repo: "${firstRepo}"`);
    }
  });

  // ── 6. List branches ──────────────────────────────────────────────────
  await test("GET {project}/_apis/git/repositories/{repo}/refs -- lists branches", async () => {
    if (!firstProject || !firstRepo) throw new Error("skipped -- no repo available");
    const raw = await adoRest(
      HOST,
      `${encodeURIComponent(firstProject)}/_apis/git/repositories/${encodeURIComponent(firstRepo)}/refs`,
      { query: { filter: "heads" } },
    );
    const data = JSON.parse(raw);
    assert(Array.isArray(data.value), "value should be an array");
    console.log(`        Found ${data.value.length} branches in "${firstRepo}"`);
    if (data.value.length > 0) {
      console.log(`        First branch: ${data.value[0].name}`);
    }
  });

  // ── 7. List commits ───────────────────────────────────────────────────
  await test("GET {project}/_apis/git/repositories/{repo}/commits -- lists commits", async () => {
    if (!firstProject || !firstRepo) throw new Error("skipped -- no repo available");
    const raw = await adoRest(
      HOST,
      `${encodeURIComponent(firstProject)}/_apis/git/repositories/${encodeURIComponent(firstRepo)}/commits`,
      { query: { "searchCriteria.$top": 5 } },
    );
    const data = JSON.parse(raw);
    assert(Array.isArray(data.value), "value should be an array");
    console.log(`        Found ${data.value.length} commits (top 5)`);
    if (data.value.length > 0) {
      const c = data.value[0];
      console.log(`        Latest: "${c.comment?.slice(0, 60)}" by ${c.author?.name}`);
    }
  });

  // ── 8. List pull requests ─────────────────────────────────────────────
  await test("GET {project}/_apis/git/pullrequests -- lists PRs", async () => {
    if (!firstProject) throw new Error("skipped -- no project available");
    const raw = await adoRest(
      HOST,
      `${encodeURIComponent(firstProject)}/_apis/git/pullrequests`,
      { query: { "searchCriteria.status": "active" } },
    );
    const data = JSON.parse(raw);
    assert(Array.isArray(data.value), "value should be an array");
    console.log(`        Found ${data.value.length} active PRs`);
  });

  // ── 9. WIQL work items query ──────────────────────────────────────────
  await test("POST _apis/wit/wiql -- queries work items", async () => {
    if (!firstProject) throw new Error("skipped -- no project available");
    const wiql = `SELECT [System.Id], [System.Title] FROM WorkItems WHERE [System.TeamProject] = '${firstProject}' ORDER BY [System.ChangedDate] DESC`;
    const raw = await adoRest(
      HOST,
      `${encodeURIComponent(firstProject)}/_apis/wit/wiql`,
      {
        method: "POST",
        body: { query: wiql },
        query: { $top: 5 },
      },
    );
    const data = JSON.parse(raw);
    assert(Array.isArray(data.workItems), "response should have workItems array");
    console.log(`        Found ${data.workItems.length} work items (top 5)`);
  });

  // ── 10. Build definitions (pipeline details) ──────────────────────────
  await test("GET {project}/_apis/build/definitions -- lists build definitions", async () => {
    if (!firstProject) throw new Error("skipped -- no project available");
    const raw = await adoRest(
      HOST,
      `${encodeURIComponent(firstProject)}/_apis/build/definitions`,
      { query: { $top: 3 } },
    );
    const data = JSON.parse(raw);
    assert(data.count !== undefined, "response should have count");
    console.log(`        Found ${data.count} build definitions`);
    if (data.value && data.value.length > 0) {
      const d = data.value[0];
      console.log(`        First: "${d.name}" (id=${d.id})`);
    }
  });

  // ── Summary ───────────────────────────────────────────────────────────
  console.log(`\n=== Results: ${passed} passed, ${failed} failed ===\n`);
  if (errors.length > 0) {
    console.log("Failures:");
    for (const e of errors) {
      console.log(`  - ${e}`);
    }
  }

  process.exit(failed > 0 ? 1 : 0);
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(2);
});
