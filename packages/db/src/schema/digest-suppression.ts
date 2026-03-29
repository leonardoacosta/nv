import { index, integer, pgTable, text, timestamp } from "drizzle-orm/pg-core";

export const digestSuppression = pgTable("digest_suppression", {
  hash: text("hash").primaryKey(),
  source: text("source").notNull(),
  priority: integer("priority").notNull(),
  lastSentAt: timestamp("last_sent_at", { withTimezone: true }).notNull(),
  expiresAt: timestamp("expires_at", { withTimezone: true }).notNull(),
  createdAt: timestamp("created_at").notNull().defaultNow(),
}, (table) => ({
  expiresAtIdx: index("digest_suppression_expires_at_idx").on(table.expiresAt),
  sourceIdx: index("digest_suppression_source_idx").on(table.source),
}));

export type DigestSuppression = typeof digestSuppression.$inferSelect;
export type NewDigestSuppression = typeof digestSuppression.$inferInsert;
