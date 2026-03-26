import { integer, jsonb, pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";

export const diary = pgTable("diary", {
  id: uuid("id").primaryKey().defaultRandom(),
  triggerType: text("trigger_type").notNull(),
  triggerSource: text("trigger_source").notNull(),
  channel: text("channel").notNull(),
  slug: text("slug").notNull(),
  content: text("content").notNull(),
  toolsUsed: jsonb("tools_used"),
  tokensIn: integer("tokens_in"),
  tokensOut: integer("tokens_out"),
  responseLatencyMs: integer("response_latency_ms"),
  createdAt: timestamp("created_at").notNull().defaultNow(),
});

export type DiaryEntry = typeof diary.$inferSelect;
export type NewDiaryEntry = typeof diary.$inferInsert;
