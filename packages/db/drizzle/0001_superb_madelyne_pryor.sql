CREATE TABLE "briefings" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"generated_at" timestamp with time zone DEFAULT now() NOT NULL,
	"content" text NOT NULL,
	"sources_status" jsonb DEFAULT '{}'::jsonb NOT NULL,
	"suggested_actions" jsonb DEFAULT '[]'::jsonb NOT NULL
);
