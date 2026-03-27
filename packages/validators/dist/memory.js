import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { memory } from "@nova/db";
import { z } from "zod/v4";
// Layer 1: Drizzle-derived base schemas
export const insertMemorySchema = createInsertSchema(memory, {
    embedding: () => z.array(z.number()).optional(),
});
export const selectMemorySchema = createSelectSchema(memory, {
    embedding: () => z.array(z.number()).nullable(),
});
// Layer 2: DTO schemas
export const createMemorySchema = insertMemorySchema.omit({
    id: true,
    updatedAt: true,
}).extend({
    topic: z.string().min(1),
    content: z.string().min(1),
});
export const updateMemorySchema = z.object({
    content: z.string().min(1),
});
//# sourceMappingURL=memory.js.map