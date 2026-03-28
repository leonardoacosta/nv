/**
 * Parser for the memory `people` topic.
 *
 * The people topic is a freeform text blob written by Nova. This parser uses
 * heuristic matching to extract structured PersonProfile records from it.
 * It is designed to degrade gracefully: unrecognized content is captured as
 * raw notes rather than failing.
 *
 * Moved from apps/dashboard/lib/entity-resolution/people-parser.ts so that
 * the API package can use it for server-side materialization and resolution.
 */
export interface PersonProfile {
    name: string;
    channelIds: Record<string, string>;
    role: string | null;
    notes: string;
}
/**
 * Parse the `people` memory topic text blob into structured PersonProfile[].
 *
 * Strategy:
 * 1. Split the content into sections, each beginning with a name header line.
 * 2. For each section, extract channel IDs, role, and notes.
 * 3. Sections with no identifiable name are skipped.
 */
export declare function parsePeopleMemory(content: string): PersonProfile[];
//# sourceMappingURL=people-parser.d.ts.map