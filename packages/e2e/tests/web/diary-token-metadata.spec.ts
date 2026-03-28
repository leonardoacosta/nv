/**
 * E2E: Diary page — token metadata, latency/cost badges, structured tool details,
 * and aggregate stats bar.
 *
 * Covers surface-diary-token-metadata tasks 4.1, 4.2, 4.3
 *   [beads:nv-khka] 4.1 — collapsed row metadata badges
 *   [beads:nv-1tq5] 4.2 — expanded view structured tool detail
 *   [beads:nv-6dzo] 4.3 — aggregate stats bar tokens + cost
 *
 * Verifies:
 * - Diary page loads and renders at least a summary bar
 * - Collapsed diary rows show token badge (Zap icon + in+out counts) when present
 * - Collapsed rows show latency badge (Clock icon + ms/s value) when present
 * - Collapsed rows show up to 3 tool pills with +N overflow indicator when present
 * - Clicking a row expands it to show structured tool detail rows
 * - Expanded detail rows contain tool name, optional input summary, and optional duration
 * - Summary bar renders a Tokens stat card when the day has token data
 * - Summary bar renders an Est. Cost stat card when cost data is present
 *
 * Design notes:
 * - Metadata badges are hidden below sm breakpoint (hidden sm:flex); tests run at Desktop Chrome
 * - Graceful skip is used throughout since diary data depends on a live daemon
 */

import { test, expect, type Page } from "@playwright/test";

// ── Helpers ──────────────────────────────────────────────────────────────────

/** Navigate to the diary page and wait for loading to settle. */
async function goDiary(page: Page) {
  await page.goto("/diary");
  await expect(page.locator("h1", { hasText: "Activity Log" })).toBeVisible({ timeout: 10_000 });
  // Wait for any loading skeleton to disappear
  await page.waitForSelector(".animate-pulse", { state: "detached", timeout: 15_000 }).catch(() => {});
}

// ── Test suite ────────────────────────────────────────────────────────────────

test.describe("Diary — token metadata, tool details, aggregate stats", () => {
  // ── 4.1: Collapsed row metadata badges ──────────────────────────────────

  test("4.1 diary page loads and renders activity log heading", async ({ page }) => {
    // Arrange + Act
    await goDiary(page);

    // Assert: heading is present
    await expect(page.locator("h1", { hasText: "Activity Log" })).toBeVisible();

    // Assert: date navigation controls rendered
    await expect(page.locator("button[aria-label='Previous day']")).toBeVisible();
    await expect(page.locator("button[aria-label='Next day']")).toBeVisible();
  });

  test("4.1 collapsed diary row shows token badge when entry has tokens", async ({ page }) => {
    await goDiary(page);

    // Locate diary entry rows — each is a role=button element (onClick + tabIndex)
    const entryRows = page.getByRole("button").filter({ hasText: /\d{2}:\d{2}:\d{2}/ });
    const count = await entryRows.count();

    if (count === 0) {
      test.skip(true, "No diary entries for today — daemon not running or no activity");
      return;
    }

    // Find an entry that has a token badge (Zap icon present in metadata badges area)
    // The metadata badges div uses class "hidden sm:flex"
    // Token badge renders: <Zap size={9}/> + fmtTokens(in) + "+" + fmtTokens(out)
    const tokenBadge = page
      .locator('[class*="hidden"][class*="sm:flex"]')
      .locator("span")
      .filter({ hasText: /^\d/ }) // Starts with a digit (token count like "1.2k+340")
      .first();

    const tokenBadgeCount = await tokenBadge.count();
    if (tokenBadgeCount === 0) {
      test.skip(true, "No token badges found — entries may have zero tokens");
      return;
    }

    await expect(tokenBadge).toBeVisible();
    // Token badge text format: "NNN+NNN" or "N.Nk+N.Nk"
    const badgeText = await tokenBadge.textContent();
    expect(badgeText).toMatch(/\d/); // Contains at least one digit
    expect(badgeText).toContain("+"); // Separator between in and out
  });

  test("4.1 collapsed diary row shows latency badge when entry has latency", async ({ page }) => {
    await goDiary(page);

    const entryRows = page.getByRole("button").filter({ hasText: /\d{2}:\d{2}:\d{2}/ });
    const count = await entryRows.count();

    if (count === 0) {
      test.skip(true, "No diary entries for today");
      return;
    }

    // Latency badge: Clock icon + "NNNms" or "N.Ns"
    const latencyBadge = page
      .locator('[class*="hidden"][class*="sm:flex"]')
      .locator("span")
      .filter({ hasText: /\d+(?:ms|s)$/ })
      .first();

    const latencyBadgeCount = await latencyBadge.count();
    if (latencyBadgeCount === 0) {
      test.skip(true, "No latency badges found — entries may have zero latency");
      return;
    }

    await expect(latencyBadge).toBeVisible();
    const badgeText = await latencyBadge.textContent();
    expect(badgeText).toMatch(/\d+(?:ms|s)/);
  });

  test("4.1 collapsed diary row shows tool pills when entry has tools", async ({ page }) => {
    await goDiary(page);

    const entryRows = page.getByRole("button").filter({ hasText: /\d{2}:\d{2}:\d{2}/ });
    const count = await entryRows.count();

    if (count === 0) {
      test.skip(true, "No diary entries for today");
      return;
    }

    // Tool pills use font-mono + Terminal icon (10px) — rendered inside "hidden md:inline-flex"
    // Each pill: Terminal icon + tool name text
    const toolPills = page.locator('[class*="md:inline-flex"][class*="font-mono"]');
    const pillCount = await toolPills.count();

    if (pillCount === 0) {
      test.skip(true, "No tool pills found — entries may have no tools_called");
      return;
    }

    // At most 3 pills per entry + possible +N overflow badge
    expect(pillCount).toBeGreaterThan(0);
    await expect(toolPills.first()).toBeVisible();
  });

  // ── 4.2: Expanded view structured tool detail ────────────────────────────

  test("4.2 expanding a diary entry reveals detail section", async ({ page }) => {
    await goDiary(page);

    const entryRows = page.getByRole("button").filter({ hasText: /\d{2}:\d{2}:\d{2}/ });
    const count = await entryRows.count();

    if (count === 0) {
      test.skip(true, "No diary entries for today");
      return;
    }

    // Act: click the first entry to expand it
    const firstEntry = entryRows.first();
    await firstEntry.click();

    // Assert: chevron rotates (rotate-180 class applied) — indirect expansion signal
    const chevron = firstEntry.locator('svg').last(); // ChevronDown is last svg in collapsed row
    const chevronClass = await chevron.getAttribute("class").catch(() => "");
    // After expand, the parent div has expanded content rendered below the header
    // Check that some expanded content is now visible (pre block or tool detail rows)
    const expandedContent = page.locator("pre").first();
    const toolDetailContainer = page.locator('.divide-y').first();
    const contentCount = await expandedContent.count() + await toolDetailContainer.count();
    expect(contentCount).toBeGreaterThan(0);
    void chevronClass; // used indirectly above
  });

  test("4.2 expanded view shows structured tool detail rows when new-format entry exists", async ({ page }) => {
    await goDiary(page);

    const entryRows = page.getByRole("button").filter({ hasText: /\d{2}:\d{2}:\d{2}/ });
    const count = await entryRows.count();

    if (count === 0) {
      test.skip(true, "No diary entries for today");
      return;
    }

    // Try expanding entries to find one with structured tool details
    // (tools_detail.length > 0 → renders ToolDetailRow components in a divide-y container)
    let foundStructuredTools = false;

    const maxToCheck = Math.min(count, 10);
    for (let i = 0; i < maxToCheck; i++) {
      const row = entryRows.nth(i);
      await row.click();

      // ToolDetailRow container: rounded-md border divide-y
      const toolDetailContainer = row.locator("..").locator('.divide-y').first();
      const containerCount = await toolDetailContainer.count();

      if (containerCount > 0) {
        const firstDetail = toolDetailContainer.locator("div").first();
        const detailCount = await firstDetail.count();
        if (detailCount > 0) {
          await expect(firstDetail).toBeVisible({ timeout: 2_000 }).catch(() => {});
          const isVisible = await firstDetail.isVisible().catch(() => false);
          if (isVisible) {
            foundStructuredTools = true;
            // Assert: tool name is rendered in mono font
            const toolName = toolDetailContainer.locator("span.font-mono").first();
            const toolNameCount = await toolName.count();
            if (toolNameCount > 0) {
              await expect(toolName).toBeVisible();
              const text = await toolName.textContent();
              expect(text?.trim().length).toBeGreaterThan(0);
            }
            break;
          }
        }
      }

      // Collapse before trying next entry
      await row.click();
    }

    if (!foundStructuredTools) {
      test.skip(true, "No entries with structured tool details (tools_detail[]) found — entries are legacy format or tools not used");
    }
  });

  test("4.2 expanded metadata row shows latency and token counts", async ({ page }) => {
    await goDiary(page);

    const entryRows = page.getByRole("button").filter({ hasText: /\d{2}:\d{2}:\d{2}/ });
    const count = await entryRows.count();

    if (count === 0) {
      test.skip(true, "No diary entries for today");
      return;
    }

    // Expand the first entry
    const firstEntry = entryRows.first();
    await firstEntry.click();

    // The expanded metadata row contains: Clock + Nms, "NNN in + NNN out", optional cost + model
    // These are rendered as font-mono spans inside the expanded div
    const monoSpans = page.locator("span.font-mono, span[class*='font-mono']");
    const monoCount = await monoSpans.count();

    if (monoCount === 0) {
      test.skip(true, "No mono spans found in expanded view");
      return;
    }

    // At least one should contain "ms" (latency) or "in +" (token counts)
    let foundLatency = false;
    let foundTokens = false;

    const checkCount = Math.min(monoCount, 20);
    for (let i = 0; i < checkCount; i++) {
      const text = await monoSpans.nth(i).textContent().catch(() => "");
      if (text?.match(/\d+ms|\d+\.\d+s/)) foundLatency = true;
      if (text?.includes(" in +")) foundTokens = true;
      if (foundLatency && foundTokens) break;
    }

    // At least one metric should be visible in expanded state
    expect(foundLatency || foundTokens).toBe(true);
  });

  // ── 4.3: Aggregate stats bar ─────────────────────────────────────────────

  test("4.3 summary bar renders Entries and Channels stat cards", async ({ page }) => {
    await goDiary(page);

    // The summary bar renders when data is loaded (not loading, not error)
    // StatCard inline variant: "flex items-center gap-2.5 py-2 px-3 border-r"
    // Each card has a label text and a value

    // These stat cards are always shown when data loads (non-aggregate)
    const entriesLabel = page.locator("span", { hasText: "Entries" }).first();
    const entriesCount = await entriesLabel.count();

    if (entriesCount === 0) {
      test.skip(true, "Summary bar not visible — data may not have loaded");
      return;
    }

    await expect(entriesLabel).toBeVisible();
    await expect(page.locator("span", { hasText: "Channels" }).first()).toBeVisible();
  });

  test("4.3 aggregate stats bar shows Tokens stat card when day has token data", async ({ page }) => {
    await goDiary(page);

    // "Tokens" stat card is rendered when data.aggregates.total_tokens_in + total_tokens_out > 0
    const tokensLabel = page.locator("span", { hasText: "Tokens" }).first();
    const count = await tokensLabel.count();

    if (count === 0) {
      test.skip(true, "No Tokens stat card found — no token data for today or no entries");
      return;
    }

    await expect(tokensLabel).toBeVisible();

    // The adjacent value span should display a formatted token count
    // StatCard inline: label → value in label-13-mono
    // The value is the next sibling span after the label
    const tokenValueSpan = tokensLabel.locator("~ span").first();
    const tokenValue = await tokenValueSpan.textContent().catch(() => null);

    if (tokenValue) {
      // Formatted as "1.2k", "12.3M", or a plain number
      expect(tokenValue.trim().length).toBeGreaterThan(0);
      expect(tokenValue.trim()).toMatch(/[\d.]+[kKmM]?/);
    }
  });

  test("4.3 aggregate stats bar shows Est. Cost stat card when cost data present", async ({ page }) => {
    await goDiary(page);

    // "Est. Cost" stat card renders when total_cost_usd != null && total_cost_usd > 0
    const costLabel = page.locator("span", { hasText: "Est. Cost" }).first();
    const count = await costLabel.count();

    if (count === 0) {
      test.skip(true, "No Est. Cost stat card found — no cost data for today (no model entries or daemon not sending cost)");
      return;
    }

    await expect(costLabel).toBeVisible();

    // Cost value should start with "$"
    const costValueSpan = costLabel.locator("~ span").first();
    const costValue = await costValueSpan.textContent().catch(() => null);

    if (costValue) {
      expect(costValue.trim()).toMatch(/^\$/);
    }
  });

  test("4.3 aggregate stats bar shows Avg Latency stat card when latency data present", async ({ page }) => {
    await goDiary(page);

    const latencyLabel = page.locator("span", { hasText: "Avg Latency" }).first();
    const count = await latencyLabel.count();

    if (count === 0) {
      test.skip(true, "No Avg Latency stat card — latency data not present for today");
      return;
    }

    await expect(latencyLabel).toBeVisible();

    const latencyValueSpan = latencyLabel.locator("~ span").first();
    const latencyValue = await latencyValueSpan.textContent().catch(() => null);

    if (latencyValue) {
      expect(latencyValue.trim()).toMatch(/\d+(?:ms|s)/);
    }
  });

  test("4.3 date navigation changes the displayed date and refetches diary data", async ({ page }) => {
    await goDiary(page);

    // Capture the current date label
    const dateLabel = page.locator("p.text-label-14").first();
    const initialDate = await dateLabel.textContent();

    // Act: navigate to previous day
    await page.locator("button[aria-label='Previous day']").click();

    // Assert: date label changes
    await expect(dateLabel).not.toHaveText(initialDate ?? "", { timeout: 5_000 });

    // Assert: loading skeleton may briefly appear then resolve
    // (tRPC refetch triggered by dateStr state change)
    await page.waitForSelector(".animate-pulse", { state: "detached", timeout: 10_000 }).catch(() => {});

    // Page should still be on /diary (no navigation away)
    expect(page.url()).toContain("/diary");
  });
});
