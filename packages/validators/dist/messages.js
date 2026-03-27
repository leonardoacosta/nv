import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { messages } from "@nova/db";
import { z } from "zod/v4";
import { dateRangeSchema } from "./common.js";
// Layer 1: Drizzle-derived base schemas
export const insertMessageSchema = createInsertSchema(messages, {
    embedding: () => z.array(z.number()).optional(),
    metadata: () => z.record(z.string(), z.unknown()).nullable().optional(),
});
export const selectMessageSchema = createSelectSchema(messages, {
    embedding: () => z.array(z.number()).nullable(),
    metadata: () => z.record(z.string(), z.unknown()).nullable(),
});
// Layer 2: Business-logic DTOs
export const createMessageSchema = insertMessageSchema.omit({
    id: true,
    createdAt: true,
});
export const messageFilterSchema = z.object({
    channel: z.string().optional(),
    sender: z.string().optional(),
    dateRange: dateRangeSchema.optional(),
});
//# sourceMappingURL=messages.js.map