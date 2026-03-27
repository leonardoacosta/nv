import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { contacts } from "@nova/db";
import { z } from "zod/v4";
// Layer 1: Drizzle-derived base schemas
export const insertContactSchema = createInsertSchema(contacts, {
    channelIds: () => z.record(z.string(), z.string()).default({}),
});
export const selectContactSchema = createSelectSchema(contacts, {
    channelIds: () => z.record(z.string(), z.string()),
});
// Layer 2: DTO schemas
export const createContactSchema = insertContactSchema.omit({
    id: true,
    createdAt: true,
}).extend({
    name: z.string().min(1),
    channelIds: z.record(z.string(), z.string()),
});
export const updateContactSchema = createContactSchema.partial();
//# sourceMappingURL=contacts.js.map