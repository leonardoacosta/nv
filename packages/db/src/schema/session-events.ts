import { index, jsonb, pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";
import { sessions } from "./sessions.js";

export const sessionEvents = pgTable(
  "session_events",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    sessionId: uuid("session_id")
      .notNull()
      .references(() => sessions.id, { onDelete: "cascade" }),
    eventType: text("event_type").notNull(),
    direction: text("direction"),
    content: text("content"),
    metadata: jsonb("metadata"),
    createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
  },
  (table) => [index("session_events_session_created_idx").on(table.sessionId, table.createdAt)],
);

export type SessionEvent = typeof sessionEvents.$inferSelect;
export type NewSessionEvent = typeof sessionEvents.$inferInsert;
