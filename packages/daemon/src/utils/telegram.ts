/**
 * Shared Telegram utility constants and helpers.
 * Import from here instead of redeclaring per-file.
 */

/** Maximum Telegram message length in characters. */
export const TELEGRAM_MAX_LEN = 4096;

/**
 * Split text into Telegram-safe chunks at 4096-char boundaries.
 *
 * Algorithm:
 * - If remaining text fits within TELEGRAM_MAX_LEN, push as-is.
 * - Otherwise find the last newline before the limit. If it is at or past
 *   50% of the limit, split there (clean paragraph break).
 * - If no good newline exists before 50%, hard-split at the limit.
 * - Strip the leading newline from the remainder to avoid blank-line chunks.
 */
export function splitForTelegram(text: string, maxLen: number = TELEGRAM_MAX_LEN): string[] {
  const chunks: string[] = [];
  let remaining = text;

  while (remaining.length > 0) {
    if (remaining.length <= maxLen) {
      chunks.push(remaining);
      break;
    }

    let splitAt = remaining.lastIndexOf("\n", maxLen);
    if (splitAt < maxLen * 0.5) splitAt = maxLen; // no good newline — hard split

    chunks.push(remaining.slice(0, splitAt));
    remaining = remaining.slice(splitAt).replace(/^\n/, "");
  }

  return chunks;
}
