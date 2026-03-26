import { boolean, pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";

export const reminders = pgTable("reminders", {
  id: uuid("id").primaryKey().defaultRandom(),
  message: text("message").notNull(),
  dueAt: timestamp("due_at", { withTimezone: true }).notNull(),
  channel: text("channel").notNull(),
  createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
  deliveredAt: timestamp("delivered_at", { withTimezone: true }),
  cancelled: boolean("cancelled").notNull().default(false),
  obligationId: uuid("obligation_id"),
});

export type Reminder = typeof reminders.$inferSelect;
export type NewReminder = typeof reminders.$inferInsert;
