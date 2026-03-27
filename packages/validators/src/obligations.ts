import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { obligations } from "@nova/db";
import { z } from "zod/v4";

// Layer 1: Drizzle-derived base schemas
export const insertObligationSchema = createInsertSchema(obligations);
export const selectObligationSchema = createSelectSchema(obligations);

// Layer 2: Business-logic enums
export const obligationStatusEnum = z.enum([
  "open",
  "in_progress",
  "done",
  "cancelled",
]);
export type ObligationStatus = z.infer<typeof obligationStatusEnum>;

// Layer 3: DTO schemas
export const createObligationSchema = insertObligationSchema
  .omit({
    id: true,
    createdAt: true,
    updatedAt: true,
    attemptCount: true,
    lastAttemptAt: true,
  })
  .extend({
    status: obligationStatusEnum.default("open"),
    priority: z.number().int().min(0).max(4).default(2),
    owner: z.string().default("nova"),
    sourceChannel: z.string().default("dashboard"),
  });

export const updateObligationSchema = createObligationSchema
  .partial()
  .omit({ detectedAction: true, sourceChannel: true });

export const obligationFilterSchema = z.object({
  status: obligationStatusEnum.optional(),
  owner: z.string().optional(),
  projectCode: z.string().optional(),
});

// Type inference
export type CreateObligationInput = z.infer<typeof createObligationSchema>;
export type UpdateObligationInput = z.infer<typeof updateObligationSchema>;
export type ObligationFilter = z.infer<typeof obligationFilterSchema>;
