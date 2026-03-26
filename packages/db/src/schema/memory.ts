import { customType, pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";

const vector = customType<{ data: number[]; driverData: string }>({
  dataType(config) {
    const dimensions = (config as { dimensions?: number } | undefined)?.dimensions ?? 1536;
    return `vector(${dimensions})`;
  },
  toDriver(value: number[]): string {
    return `[${value.join(",")}]`;
  },
  fromDriver(value: string): number[] {
    return value
      .replace(/^\[|\]$/g, "")
      .split(",")
      .map(Number);
  },
});

export const memory = pgTable("memory", {
  id: uuid("id").primaryKey().defaultRandom(),
  topic: text("topic").notNull().unique(),
  content: text("content").notNull(),
  embedding: vector("embedding", { dimensions: 1536 }),
  updatedAt: timestamp("updated_at").notNull().defaultNow(),
});

export type Memory = typeof memory.$inferSelect;
export type NewMemory = typeof memory.$inferInsert;
