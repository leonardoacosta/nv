import { createInsertSchema, createSelectSchema } from "drizzle-zod";
import { sessionEvents } from "@nova/db";
import { z } from "zod/v4";
// Layer 1: Drizzle-derived base schemas (append-only table, no DTOs)
export const insertSessionEventSchema = createInsertSchema(sessionEvents, {
    metadata: () => z.record(z.string(), z.unknown()).nullable().optional(),
});
export const selectSessionEventSchema = createSelectSchema(sessionEvents, {
    metadata: () => z.record(z.string(), z.unknown()).nullable(),
});
//# sourceMappingURL=session-events.js.map