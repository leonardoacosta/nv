import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { briefings } from "@nova/db";
import { z } from "zod/v4";

// Layer 1: Drizzle-derived base schemas
export const insertBriefingSchema = createInsertSchema(briefings, {
  sourcesStatus: () => z.record(z.string(), z.unknown()).default({}),
  suggestedActions: () => z.array(z.unknown()).default([]),
});
export const selectBriefingSchema = createSelectSchema(briefings, {
  sourcesStatus: () => z.record(z.string(), z.unknown()),
  suggestedActions: () => z.array(z.unknown()),
});

// Layer 2: DTO schemas
export const createBriefingSchema = insertBriefingSchema.omit({
  id: true,
  generatedAt: true,
}).extend({
  content: z.string().min(1),
});

// Type inference
export type CreateBriefingInput = z.infer<typeof createBriefingSchema>;
