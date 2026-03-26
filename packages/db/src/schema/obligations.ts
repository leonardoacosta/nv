import { integer, pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";

export const obligations = pgTable("obligations", {
  id: uuid("id").primaryKey().defaultRandom(),
  detectedAction: text("detected_action").notNull(),
  owner: text("owner").notNull(),
  status: text("status").notNull(),
  priority: integer("priority").notNull(),
  projectCode: text("project_code"),
  sourceChannel: text("source_channel").notNull(),
  sourceMessage: text("source_message"),
  deadline: timestamp("deadline"),
  lastAttemptAt: timestamp("last_attempt_at"),
  createdAt: timestamp("created_at").notNull().defaultNow(),
  updatedAt: timestamp("updated_at").notNull().defaultNow(),
});

export type Obligation = typeof obligations.$inferSelect;
export type NewObligation = typeof obligations.$inferInsert;
