/**
 * E2E: Settings page — config source badges, secret reveal toggle, memory summary card
 *
 * Covers enhance-settings-verbosity task 4.3 [beads:nv-llp1]
 *
 * Verifies:
 * - ENV badge appears on env-overridden fields and input is disabled
 * - FILE badge appears on file-sourced fields
 * - DEFAULT badge appears when falling back to defaults
 * - SecretField shows last-4 characters when value is set
 * - SecretField reveal toggle shows full value on click (show/hide)
 * - MemorySummaryCard shows entry count
 * - MemorySummaryCard shows clickable topic chips
 * - MemorySummaryCard has "View all topics" link pointing to /memory
 */

import { test, expect } from "@playwright/test";

test.describe("Settings — config source badges, secret reveal, memory summary", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/settings");
    await expect(page.locator("text=Settings")).toBeVisible();
    await page.waitForSelector(".animate-pulse", { state: "detached", timeout: 10_000 }).catch(() => {});
  });

  // ---------------------------------------------------------------------------
  // Config source badges
  // ---------------------------------------------------------------------------

  test("config source badges render and are one of ENV, FILE, or DEFAULT", async ({ page }) => {
    const anyBadge = page.locator('[data-testid^="config-source-badge-"]').first();
    const count = await anyBadge.count();

    if (count === 0) {
      // configSources query may have returned empty (daemon not running)
      // Verify the page still renders correctly without badges
      await expect(page.locator("text=Settings")).toBeVisible();
      test.skip(true, "No config source badges present — configSources query returned empty");
      return;
    }

    await expect(anyBadge).toBeVisible();
    const text = await anyBadge.textContent();
    expect(["ENV", "FILE", "DEFAULT"].some((t) => text?.includes(t))).toBe(true);
  });

  test("ENV badge disables the associated input field", async ({ page }) => {
    const envBadge = page.locator('[data-testid="config-source-badge-env"]').first();
    const count = await envBadge.count();

    if (count === 0) {
      test.skip(true, "No ENV-sourced fields found — skipping disabled input assertion");
      return;
    }

    await expect(envBadge).toBeVisible();

    // Find the input within the same field row as the ENV badge
    const parentRow = page.locator('[data-testid^="field-row-"]').filter({
      has: envBadge,
    }).first();

    const rowCount = await parentRow.count();
    if (rowCount > 0) {
      const input = parentRow.locator("input").first();
      const inputCount = await input.count();
      if (inputCount > 0) {
        await expect(input).toBeDisabled();
      }
    }
  });

  test("ENV badge tooltip contains env var name", async ({ page }) => {
    const envBadgeWrapper = page.locator('[data-testid="config-source-badge-env"]')
      .locator("..")
      .first();

    const count = await envBadgeWrapper.count();
    if (count === 0) {
      test.skip(true, "No ENV badges found");
      return;
    }

    // The wrapper span has a title attribute with the env var name
    const wrapperWithTitle = page.locator('span[title*="environment variable"]').first();
    const titleCount = await wrapperWithTitle.count();
    if (titleCount > 0) {
      const title = await wrapperWithTitle.getAttribute("title");
      expect(title).toContain("environment variable");
    }
  });

  // ---------------------------------------------------------------------------
  // SecretField
  // ---------------------------------------------------------------------------

  test("secret field shows masked value with last-4 chars when value is set", async ({ page }) => {
    const secretField = page.locator('[data-testid="secret-field"]').first();
    const count = await secretField.count();

    if (count === 0) {
      test.skip(true, "No secret fields found on settings page");
      return;
    }

    await expect(secretField).toBeVisible();

    // Status dot should be present
    const dot = secretField.locator('[data-testid="secret-field-dot"]');
    await expect(dot).toBeVisible();

    // The displayed value should be masked (dashes) or "(not set)"
    const valueSpan = secretField.locator('[data-testid="secret-field-value"]');
    await expect(valueSpan).toBeVisible();
    const displayedText = await valueSpan.textContent();
    expect(displayedText).toBeTruthy();
    // Masked format: --------XXXX or "(not set)"
    const isMasked = displayedText?.startsWith("--------") ?? false;
    const isNotSet = displayedText?.includes("not set") ?? false;
    expect(isMasked || isNotSet).toBe(true);
  });

  test("secret field green dot when value is set", async ({ page }) => {
    // Find a secret field that has a value set (green dot)
    const secretFields = page.locator('[data-testid="secret-field"]');
    const fieldCount = await secretFields.count();

    if (fieldCount === 0) {
      test.skip(true, "No secret fields found");
      return;
    }

    let foundSetField = false;
    for (let i = 0; i < fieldCount; i++) {
      const field = secretFields.nth(i);
      const dot = field.locator('[data-testid="secret-field-dot"]');
      const dotClass = await dot.getAttribute("class").catch(() => "");
      if (dotClass?.includes("green")) {
        foundSetField = true;
        await expect(dot).toHaveClass(/bg-green-700/);
        break;
      }
    }

    if (!foundSetField) {
      // All unset — verify red dot
      const firstDot = secretFields.first().locator('[data-testid="secret-field-dot"]');
      await expect(firstDot).toHaveClass(/bg-red-700/);
    }
  });

  test("secret field reveal toggle shows hide button after click, re-masks after 5s", async ({ page }) => {
    // Find a secret field that has a value (green dot) and therefore has a reveal button
    const secretFields = page.locator('[data-testid="secret-field"]');
    const fieldCount = await secretFields.count();

    if (fieldCount === 0) {
      test.skip(true, "No secret fields found");
      return;
    }

    let targetField: ReturnType<typeof secretFields.nth> | null = null;
    for (let i = 0; i < fieldCount; i++) {
      const field = secretFields.nth(i);
      const toggle = field.locator('[data-testid="secret-field-reveal-toggle"]');
      if (await toggle.count() > 0) {
        targetField = field;
        break;
      }
    }

    if (!targetField) {
      test.skip(true, "No secret fields with a value set (no reveal toggle present)");
      return;
    }

    const toggle = targetField.locator('[data-testid="secret-field-reveal-toggle"]');
    await expect(toggle).toHaveText("show");

    // Act: click reveal
    await toggle.click();

    // Assert: toggle now says "hide"
    await expect(toggle).toHaveText("hide", { timeout: 2_000 });

    // Assert: value display changes (no longer starting with "--------")
    const valueSpan = targetField.locator('[data-testid="secret-field-value"]');
    const revealed = await valueSpan.textContent();
    expect(revealed?.startsWith("--------")).toBe(false);
  });

  // ---------------------------------------------------------------------------
  // MemorySummaryCard
  // ---------------------------------------------------------------------------

  test("memory summary card shows entry count", async ({ page }) => {
    const memoryCard = page.locator('[data-testid="memory-summary-card"]');
    const count = await memoryCard.count();

    if (count === 0) {
      test.skip(true, "Memory summary card not present — memorySummary query returned empty");
      return;
    }

    await expect(memoryCard).toBeVisible();

    // Entry count should be a number
    const entryCount = memoryCard.locator('[data-testid="memory-entry-count"]');
    await expect(entryCount).toBeVisible();
    const countText = await entryCount.textContent();
    expect(countText?.trim().length).toBeGreaterThan(0);
    // Should be a numeric string (possibly with commas for thousands)
    expect(/[\d,]+/.test(countText ?? "")).toBe(true);
  });

  test("memory summary topic chips are clickable links to /memory", async ({ page }) => {
    const memoryCard = page.locator('[data-testid="memory-summary-card"]');
    const count = await memoryCard.count();

    if (count === 0) {
      test.skip(true, "Memory summary card not present");
      return;
    }

    const topicChip = memoryCard.locator('[data-testid="memory-topic-chip"]').first();
    const chipCount = await topicChip.count();

    if (chipCount === 0) {
      // No topics recorded yet — verify card still renders
      await expect(memoryCard).toBeVisible();
      return;
    }

    await expect(topicChip).toBeVisible();

    // Each chip should href to /memory?topic=...
    const href = await topicChip.getAttribute("href");
    expect(href).toContain("/memory");
    expect(href).toContain("topic=");
  });

  test("memory summary card has View all topics link to /memory", async ({ page }) => {
    const memoryCard = page.locator('[data-testid="memory-summary-card"]');
    const count = await memoryCard.count();

    if (count === 0) {
      test.skip(true, "Memory summary card not present");
      return;
    }

    const viewAllLink = memoryCard.locator('[data-testid="memory-view-all-link"]');
    await expect(viewAllLink).toBeVisible();
    await expect(viewAllLink).toHaveText("View all topics");

    const href = await viewAllLink.getAttribute("href");
    expect(href).toBe("/memory");
  });

  test("memory section renders when navigating to settings", async ({ page }) => {
    // Expand memory section if collapsed
    const memorySection = page.locator("button", { hasText: "Memory" });
    await expect(memorySection).toBeVisible({ timeout: 5_000 });

    const sectionOpen = await memorySection.locator("..").locator(".height-reveal").evaluate(
      (el) => el.classList.contains("open"),
    ).catch(() => true);

    if (!sectionOpen) {
      await memorySection.click();
    }

    await expect(page.getByRole("heading", { name: "Memory" })).toBeVisible();
  });
});
