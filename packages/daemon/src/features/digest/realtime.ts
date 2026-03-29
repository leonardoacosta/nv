import type { DigestSchedulerDeps } from "./scheduler.js";
import { gatherP0Only } from "./gather.js";
import { classifyP0Only } from "./classify.js";
import { suppressItems, markItemsSent } from "./suppress.js";
import { formatP0Alert } from "./format.js";

/**
 * Check for P0 items (PIM expiry, prod CI failures) and send immediate alerts.
 * Called every 5 minutes by the scheduler. P0 bypasses quiet hours.
 */
export async function checkP0(deps: DigestSchedulerDeps): Promise<number> {
  const { pool, logger, telegram, telegramChatId, config } = deps;

  if (!telegram || !telegramChatId) return 0;

  const gatherResult = await gatherP0Only({ pool, logger });
  const p0Items = classifyP0Only(gatherResult.pimRoles, gatherResult.adoBuilds);

  if (p0Items.length === 0) {
    logger.debug("Digest P0 check: no P0 items");
    return 0;
  }

  // Apply suppression (30min cooldown for P0)
  const suppressResult = await suppressItems(p0Items, pool, config.digest);
  const { passed: unsuppressed } = suppressResult;

  if (unsuppressed.length === 0) {
    logger.debug("Digest P0 check: all P0 items suppressed (within cooldown)");
    return 0;
  }

  // Send each P0 item as a standalone urgent notification
  let sentCount = 0;

  for (const item of unsuppressed) {
    const { text, keyboard } = formatP0Alert(item);

    try {
      await telegram.sendMessage(telegramChatId, text, {
        parseMode: "Markdown",
        disablePreview: true,
        ...(keyboard ? { keyboard } : {}),
      });
      sentCount++;
    } catch (err) {
      logger.warn({ err, item: item.id }, "Digest P0: failed to send alert");
    }
  }

  // Mark items as sent so cooldown applies
  if (sentCount > 0) {
    await markItemsSent(unsuppressed, pool, config.digest);
    logger.info({ count: sentCount }, "Digest P0: sent urgent alerts");
  }

  return sentCount;
}
