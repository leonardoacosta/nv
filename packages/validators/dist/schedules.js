import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { schedules } from "@nova/db";
import { z } from "zod/v4";
// Layer 1: Drizzle-derived base schemas
export const insertScheduleSchema = createInsertSchema(schedules);
export const selectScheduleSchema = createSelectSchema(schedules);
// Layer 2: DTO schemas
export const createScheduleSchema = insertScheduleSchema.omit({
    id: true,
    createdAt: true,
    lastRunAt: true,
    enabled: true,
}).extend({
    name: z.string().min(1),
    cronExpr: z.string().min(1),
    action: z.string().min(1),
    channel: z.string().min(1),
    enabled: z.boolean().default(true),
});
export const updateScheduleSchema = z.object({
    name: z.string().min(1).optional(),
    cronExpr: z.string().min(1).optional(),
    action: z.string().min(1).optional(),
    channel: z.string().min(1).optional(),
    enabled: z.boolean().optional(),
});
//# sourceMappingURL=schedules.js.map