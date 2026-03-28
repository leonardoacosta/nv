/**
 * Project materialization: merge projects from the daemon's project registry
 * (GET /api/projects) and from projects-* memory topics. Upsert into the
 * projects table.
 *
 * Returns { created, updated, unchanged }.
 */
export interface MaterializeResult {
    created: number;
    updated: number;
    unchanged: number;
}
export declare function materializeProjects(): Promise<MaterializeResult>;
//# sourceMappingURL=materialize-projects.d.ts.map