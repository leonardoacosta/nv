import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { diary } from "@nova/db";
import { z } from "zod/v4";

// Structured tool call detail (new format)
export const toolCallDetailSchema = z.object({
  name: z.string(),
  input_summary: z.string(),
  duration_ms: z.number().nullable(),
});

export type ToolCallDetail = z.infer<typeof toolCallDetailSchema>;

// Accept both legacy string[] and new ToolCallDetail[] shapes
export const toolsUsedSchema = z.union([
  z.array(z.string()),
  z.array(toolCallDetailSchema),
]);

// Layer 1: Drizzle-derived base schemas (read-only in dashboard, no DTOs)
export const insertDiarySchema = createInsertSchema(diary, {
  toolsUsed: () => toolsUsedSchema.nullable().optional(),
});
export const selectDiarySchema = createSelectSchema(diary, {
  toolsUsed: () => toolsUsedSchema.nullable(),
});
