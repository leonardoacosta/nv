import { integer, pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";

export const sessions = pgTable("sessions", {
  id: uuid("id").primaryKey().defaultRandom(),
  project: text("project").notNull(),
  command: text("command").notNull(),
  status: text("status").notNull().default("running"),
  triggerType: text("trigger_type"),
  messageCount: integer("message_count").notNull().default(0),
  toolCount: integer("tool_count").notNull().default(0),
  startedAt: timestamp("started_at", { withTimezone: true }).notNull().defaultNow(),
  stoppedAt: timestamp("stopped_at", { withTimezone: true }),
});

export type Session = typeof sessions.$inferSelect;
export type NewSession = typeof sessions.$inferInsert;
