import { pgTable, text, timestamp, unique, uuid } from "drizzle-orm/pg-core";

export const memory = pgTable("memory", {
  id: uuid("id").primaryKey().defaultRandom(),
  topic: text("topic").notNull().unique(),
  content: text("content").notNull(),
  updatedAt: timestamp("updated_at").notNull().defaultNow(),
});

export type Memory = typeof memory.$inferSelect;
export type NewMemory = typeof memory.$inferInsert;
