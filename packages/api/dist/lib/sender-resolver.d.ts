/**
 * Batched sender resolution for message.list.
 *
 * Resolution priority (first match wins):
 *   1. Contacts table — channel_ids JSONB match
 *   2. Telegram metadata — extract first_name/last_name/username from metadata JSONB
 *   3. Memory people profiles — parse `people` memory topic and match by channel ID
 *   4. Raw fallback — return the raw sender string unchanged
 */
export type ResolutionSource = "contact" | "telegram-meta" | "memory" | "raw";
export interface SenderResolution {
    displayName: string;
    avatarInitial: string;
    source: ResolutionSource;
}
export interface SenderInput {
    raw: string;
    channel: string;
    metadata: unknown;
}
/**
 * Resolve a batch of sender+channel pairs to display names.
 *
 * All DB queries are batched — contacts and memory are loaded once and
 * reused across all senders in the request.
 *
 * @param senders - Array of unique sender descriptors from the current page
 * @returns Map keyed by `${channel}:${raw}` → SenderResolution
 */
export declare function resolveSenders(senders: SenderInput[]): Promise<Map<string, SenderResolution>>;
//# sourceMappingURL=sender-resolver.d.ts.map