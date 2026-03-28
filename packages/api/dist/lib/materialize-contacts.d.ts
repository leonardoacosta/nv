/**
 * Contact materialization: read the "people" memory topic, parse into
 * PersonProfile[], match each profile to existing contacts by channel ID
 * or name, and upsert.
 *
 * Returns { created, updated, unchanged }.
 */
export interface MaterializeResult {
    created: number;
    updated: number;
    unchanged: number;
}
export declare function materializeContacts(): Promise<MaterializeResult>;
//# sourceMappingURL=materialize-contacts.d.ts.map