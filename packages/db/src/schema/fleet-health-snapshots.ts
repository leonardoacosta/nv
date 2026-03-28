import { index, integer, pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";

export const fleetHealthSnapshots = pgTable("fleet_health_snapshots", {
  id: uuid("id").primaryKey().defaultRandom(),
  serviceName: text("service_name").notNull(),
  status: text("status").notNull(),
  latencyMs: integer("latency_ms"),
  checkedAt: timestamp("checked_at", { withTimezone: true }).notNull().defaultNow(),
}, (table) => ({
  serviceNameCheckedAtIdx: index("fleet_health_snapshots_service_name_checked_at_idx").on(table.serviceName, table.checkedAt),
}));

export type FleetHealthSnapshot = typeof fleetHealthSnapshots.$inferSelect;
export type NewFleetHealthSnapshot = typeof fleetHealthSnapshots.$inferInsert;
