import { jsonb, pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";

export const briefings = pgTable("briefings", {
  id: uuid("id").primaryKey().defaultRandom(),
  generatedAt: timestamp("generated_at", { withTimezone: true }).notNull().defaultNow(),
  content: text("content").notNull(),
  sourcesStatus: jsonb("sources_status").notNull().default({}),
  suggestedActions: jsonb("suggested_actions").notNull().default([]),
});

export type Briefing = typeof briefings.$inferSelect;
export type NewBriefing = typeof briefings.$inferInsert;
