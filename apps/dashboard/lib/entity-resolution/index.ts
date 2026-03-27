/**
 * Entity resolution library barrel export.
 *
 * Functions:
 *  - parsePeopleMemory  — parse memory `people` topic into PersonProfile[]
 *  - resolveContacts    — build sender -> displayName map from contacts + memory
 *  - enrichProjects     — enrich ApiProject[] with DB counts + memory context
 *
 * Types:
 *  - PersonProfile
 *  - ContactRow
 *  - ApiProject
 *  - EnrichedProject
 */

export { parsePeopleMemory } from "./people-parser";
export type { PersonProfile } from "./people-parser";

export { resolveContacts } from "./contact-resolver";
export type { ContactRow } from "./contact-resolver";

export { enrichProjects } from "./project-enrichment";
export type { ApiProject, EnrichedProject } from "./project-enrichment";
