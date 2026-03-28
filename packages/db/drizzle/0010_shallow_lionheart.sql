CREATE TABLE "fleet_health_snapshots" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"service_name" text NOT NULL,
	"status" text NOT NULL,
	"latency_ms" integer,
	"checked_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
ALTER TABLE "diary" ADD COLUMN "model" text;--> statement-breakpoint
ALTER TABLE "diary" ADD COLUMN "cost_usd" numeric(10, 6);--> statement-breakpoint
CREATE INDEX "fleet_health_snapshots_service_name_checked_at_idx" ON "fleet_health_snapshots" USING btree ("service_name","checked_at");