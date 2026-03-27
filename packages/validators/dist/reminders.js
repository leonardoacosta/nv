import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { reminders } from "@nova/db";
import { z } from "zod/v4";
// Layer 1: Drizzle-derived base schemas
export const insertReminderSchema = createInsertSchema(reminders);
export const selectReminderSchema = createSelectSchema(reminders);
// Layer 2: DTO schemas
export const createReminderSchema = insertReminderSchema.omit({
    id: true,
    createdAt: true,
    deliveredAt: true,
    cancelled: true,
}).extend({
    message: z.string().min(1),
    dueAt: z.coerce.date(),
    channel: z.string().min(1),
});
export const updateReminderSchema = z.object({
    message: z.string().min(1).optional(),
    dueAt: z.coerce.date().optional(),
    channel: z.string().min(1).optional(),
    cancelled: z.boolean().optional(),
});
//# sourceMappingURL=reminders.js.map