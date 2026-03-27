import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { sessions } from "@nova/db";
import { z } from "zod/v4";
import { dateRangeSchema } from "./common.js";

// Layer 1: Drizzle-derived base schemas
export const insertSessionSchema = createInsertSchema(sessions);
export const selectSessionSchema = createSelectSchema(sessions);

// Layer 2: DTO schemas
export const createSessionSchema = insertSessionSchema.omit({
  id: true,
  startedAt: true,
  stoppedAt: true,
  messageCount: true,
  toolCount: true,
  status: true,
}).extend({
  project: z.string().min(1),
  command: z.string().min(1),
});

export const sessionFilterSchema = z.object({
  project: z.string().optional(),
  status: z.string().optional(),
  dateRange: dateRangeSchema.optional(),
});

// Type inference
export type CreateSessionInput = z.infer<typeof createSessionSchema>;
export type SessionFilter = z.infer<typeof sessionFilterSchema>;
