/**
 * Telegram message metadata extraction helpers.
 *
 * Telegram Bot API Message objects embed sender info under `message.from`.
 * The messages table stores the raw Telegram update payload in the `metadata`
 * JSONB column — this module extracts a display name from it.
 */
/**
 * Extract a human-readable display name from Telegram message metadata JSONB.
 *
 * Handles two common shapes produced by the Telegram Bot API:
 *   - Nested: `{ from: { first_name, last_name?, username? } }`
 *   - Flat:   `{ first_name, last_name?, username? }` (some webhook adapters flatten it)
 *
 * Returns:
 *   - `"first_name last_name"` (trimmed) when both are available
 *   - `"first_name"` when only first name is present
 *   - `username` as final text fallback
 *   - `null` if none of the above are found
 */
export declare function extractTelegramName(metadata: Record<string, unknown> | null): string | null;
//# sourceMappingURL=telegram-metadata.d.ts.map