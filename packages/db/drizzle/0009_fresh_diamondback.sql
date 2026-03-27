ALTER TABLE "messages" ADD COLUMN "thread_id" text;--> statement-breakpoint
ALTER TABLE "messages" ADD COLUMN "reply_to_message_id" integer;--> statement-breakpoint
CREATE INDEX "messages_thread_id_idx" ON "messages" ("thread_id") WHERE "thread_id" IS NOT NULL;