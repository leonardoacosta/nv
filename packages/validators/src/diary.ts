import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { diary } from "@nova/db";
import { z } from "zod/v4";

// Layer 1: Drizzle-derived base schemas (read-only in dashboard, no DTOs)
export const insertDiarySchema = createInsertSchema(diary, {
  toolsUsed: () => z.array(z.string()).nullable().optional(),
});
export const selectDiarySchema = createSelectSchema(diary, {
  toolsUsed: () => z.array(z.string()).nullable(),
});
