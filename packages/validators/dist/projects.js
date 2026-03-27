import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { projects } from "@nova/db";
import { z } from "zod/v4";
// Layer 1: Drizzle-derived base schemas
export const insertProjectSchema = createInsertSchema(projects);
export const selectProjectSchema = createSelectSchema(projects);
// Layer 2: Business-logic enums (migrated from packages/db/src/schema/projects.ts)
export const projectCategoryEnum = z.enum([
    "work",
    "personal",
    "open_source",
    "archived",
]);
export const projectStatusEnum = z.enum([
    "active",
    "paused",
    "completed",
    "archived",
]);
// Layer 3: DTO schemas (migrated from packages/db/src/schema/projects.ts)
export const createProjectSchema = z.object({
    code: z.string().min(1),
    name: z.string().min(1),
    category: projectCategoryEnum.optional(),
    status: projectStatusEnum.optional(),
    description: z.string().optional(),
    content: z.string().optional(),
    path: z.string().optional(),
});
export const updateProjectSchema = z.object({
    name: z.string().min(1).optional(),
    category: projectCategoryEnum.optional(),
    status: projectStatusEnum.optional(),
    description: z.string().optional(),
    content: z.string().optional(),
    path: z.string().optional(),
});
//# sourceMappingURL=projects.js.map