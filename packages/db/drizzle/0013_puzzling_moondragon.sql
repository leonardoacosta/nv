CREATE TABLE "digest_suppression" (
	"hash" text PRIMARY KEY NOT NULL,
	"source" text NOT NULL,
	"priority" integer NOT NULL,
	"last_sent_at" timestamp with time zone NOT NULL,
	"expires_at" timestamp with time zone NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE INDEX "digest_suppression_expires_at_idx" ON "digest_suppression" USING btree ("expires_at");--> statement-breakpoint
CREATE INDEX "digest_suppression_source_idx" ON "digest_suppression" USING btree ("source");