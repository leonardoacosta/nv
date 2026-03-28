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
export function extractTelegramName(metadata) {
    if (!metadata)
        return null;
    // Try nested `from` object first (standard Telegram Bot API shape)
    const from = metadata["from"];
    if (from && typeof from === "object" && !Array.isArray(from)) {
        const name = buildNameFromObject(from);
        if (name)
            return name;
    }
    // Try flat shape (some adapters merge `from` fields into top-level metadata)
    return buildNameFromObject(metadata);
}
function buildNameFromObject(obj) {
    const firstName = typeof obj["first_name"] === "string" ? obj["first_name"].trim() : null;
    const lastName = typeof obj["last_name"] === "string" ? obj["last_name"].trim() : null;
    const username = typeof obj["username"] === "string" ? obj["username"].trim() : null;
    if (firstName && lastName)
        return `${firstName} ${lastName}`;
    if (firstName)
        return firstName;
    if (username)
        return username;
    return null;
}
//# sourceMappingURL=telegram-metadata.js.map