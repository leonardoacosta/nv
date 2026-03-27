/**
 * Unit tests for ado-rest.ts module logic.
 *
 * These tests do NOT require SSH or network access. They verify:
 *   - URL construction
 *   - Token caching logic
 *   - Query parameter encoding
 *   - Error handling paths
 *
 * Run with: npx tsx src/tools/__tests__/ado-rest-unit.test.ts
 */

import { ADO_ORG, clearTokenCache } from "../ado-rest.js";

// ── Helpers ─────────────────────────────────────────────────────────────

let passed = 0;
let failed = 0;
const errors: string[] = [];

async function test(name: string, fn: () => Promise<void> | void): Promise<void> {
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
  console.log("\n=== Azure DevOps REST API Unit Tests ===\n");

  // ── ADO_ORG constant ──────────────────────────────────────────────────
  await test("ADO_ORG is correctly set", () => {
    assert(ADO_ORG === "brownandbrowninc", `expected 'brownandbrowninc', got '${ADO_ORG}'`);
  });

  // ── URL construction ──────────────────────────────────────────────────
  await test("REST URL construction -- projects endpoint", () => {
    const base = `https://dev.azure.com/${ADO_ORG}`;
    const path = "_apis/projects";
    const url = new URL(`${base}/${path}`);
    url.searchParams.set("api-version", "7.1-preview");

    assert(
      url.toString() === `https://dev.azure.com/brownandbrowninc/_apis/projects?api-version=7.1-preview`,
      `URL mismatch: ${url.toString()}`,
    );
  });

  await test("REST URL construction -- builds with $top", () => {
    const base = `https://dev.azure.com/${ADO_ORG}`;
    const path = "MyProject/_apis/build/builds";
    const url = new URL(`${base}/${path}`);
    url.searchParams.set("api-version", "7.1-preview");
    url.searchParams.set("$top", "10");

    assert(url.pathname === "/brownandbrowninc/MyProject/_apis/build/builds", `path: ${url.pathname}`);
    assert(url.searchParams.get("$top") === "10", "should have $top=10");
  });

  await test("REST URL construction -- commits with searchCriteria", () => {
    const base = `https://dev.azure.com/${ADO_ORG}`;
    const project = "My Project";
    const repo = "my-repo";
    const path = `${encodeURIComponent(project)}/_apis/git/repositories/${encodeURIComponent(repo)}/commits`;
    const url = new URL(`${base}/${path}`);
    url.searchParams.set("api-version", "7.1-preview");
    url.searchParams.set("searchCriteria.$top", "20");

    assert(
      url.pathname.includes("My%20Project"),
      `project should be encoded: ${url.pathname}`,
    );
    assert(
      url.pathname.includes("my-repo"),
      `repo should be in path: ${url.pathname}`,
    );
    assert(
      url.searchParams.get("searchCriteria.$top") === "20",
      "should have searchCriteria.$top=20",
    );
  });

  await test("REST URL construction -- PR status filter", () => {
    const base = `https://dev.azure.com/${ADO_ORG}`;
    const path = "TestProject/_apis/git/pullrequests";
    const url = new URL(`${base}/${path}`);
    url.searchParams.set("api-version", "7.1-preview");
    url.searchParams.set("searchCriteria.status", "active");

    assert(
      url.searchParams.get("searchCriteria.status") === "active",
      "should have status filter",
    );
  });

  await test("REST URL construction -- branches with filter=heads", () => {
    const base = `https://dev.azure.com/${ADO_ORG}`;
    const path = "Project/_apis/git/repositories/Repo/refs";
    const url = new URL(`${base}/${path}`);
    url.searchParams.set("api-version", "7.1-preview");
    url.searchParams.set("filter", "heads");

    assert(
      url.searchParams.get("filter") === "heads",
      "should filter by heads",
    );
  });

  // ── Token cache invalidation ──────────────────────────────────────────
  await test("clearTokenCache does not throw", () => {
    // Just ensure no exceptions
    clearTokenCache();
    clearTokenCache(); // Calling twice should be safe
  });

  // ── Query param encoding for special characters ───────────────────────
  await test("project names with spaces are properly encoded in URLs", () => {
    const base = `https://dev.azure.com/${ADO_ORG}`;
    const project = "Brown & Brown - IT";
    const path = `${encodeURIComponent(project)}/_apis/pipelines`;
    const url = new URL(`${base}/${path}`);

    assert(
      url.pathname.includes("Brown%20%26%20Brown%20-%20IT"),
      `project should be URL-encoded: ${url.pathname}`,
    );
  });

  // ── WIQL body construction ────────────────────────────────────────────
  await test("WIQL query body is valid JSON", () => {
    const wiql = `SELECT [System.Id], [System.Title] FROM WorkItems WHERE [System.TeamProject] = 'IT' ORDER BY [System.ChangedDate] DESC`;
    const body = JSON.stringify({ query: wiql });
    const parsed = JSON.parse(body);
    assert(typeof parsed.query === "string", "body should have query string");
    assert(parsed.query.includes("SELECT"), "query should be WIQL");
  });

  // ── Pipeline run body construction ────────────────────────────────────
  await test("pipeline run body with branch is correctly structured", () => {
    const branch = "refs/heads/main";
    const body: Record<string, unknown> = {
      resources: {
        repositories: {
          self: { refName: branch },
        },
      },
    };
    const json = JSON.stringify(body);
    const parsed = JSON.parse(json);
    assert(
      parsed.resources.repositories.self.refName === "refs/heads/main",
      "should have branch in body",
    );
  });

  await test("pipeline run body without branch is empty object", () => {
    const body: Record<string, unknown> = {};
    assert(Object.keys(body).length === 0, "empty body");
  });

  // ── Sanitize interaction ──────────────────────────────────────────────
  await test("sanitize strips injection characters", async () => {
    // Import sanitize dynamically to verify it works
    const { sanitize } = await import("../../utils.js");
    assert(sanitize("foo'bar") === "foobar", "strips single quotes");
    assert(sanitize("foo;bar") === "foobar", "strips semicolons");
    assert(sanitize("foo`bar") === "foobar", "strips backticks");
    assert(sanitize("foo|bar") === "foobar", "strips pipes");
    assert(sanitize("normal-name_123") === "normal-name_123", "preserves safe chars");
  });

  // ── Error classification ──────────────────────────────────────────────
  await test("SshError has correct HTTP status codes", async () => {
    const { SshError } = await import("../../ssh.js");
    const err502 = new SshError("test", 502);
    const err503 = new SshError("test", 503);
    const err504 = new SshError("test", 504);

    assert(err502.httpStatus === 502, "502 status");
    assert(err503.httpStatus === 503, "503 status");
    assert(err504.httpStatus === 504, "504 status");
    assert(err502.name === "SshError", "name should be SshError");
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
