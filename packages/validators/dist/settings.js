import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { settings } from "@nova/db";
import { z } from "zod/v4";
// Layer 1: Drizzle-derived base schemas
export const insertSettingSchema = createInsertSchema(settings);
export const selectSettingSchema = createSelectSchema(settings);
// Layer 2: DTO schemas
export const upsertSettingSchema = z.object({
    key: z.string().min(1),
    value: z.string().min(1),
});
//# sourceMappingURL=settings.js.map