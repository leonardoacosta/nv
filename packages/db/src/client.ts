import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";
import { messages } from "./schema/messages.js";
import { obligations } from "./schema/obligations.js";
import { contacts } from "./schema/contacts.js";
import { diary } from "./schema/diary.js";
import { memory } from "./schema/memory.js";
import { briefings } from "./schema/briefings.js";
import { reminders } from "./schema/reminders.js";
import { schedules } from "./schema/schedules.js";
import { sessions } from "./schema/sessions.js";
import { sessionEvents } from "./schema/session-events.js";
import { projects } from "./schema/projects.js";
import { settings } from "./schema/settings.js";

const connectionString = process.env.DATABASE_URL;

if (!connectionString) {
  throw new Error("DATABASE_URL environment variable is required");
}

const queryClient = postgres(connectionString);

const schema = { messages, obligations, contacts, diary, memory, briefings, reminders, schedules, sessions, sessionEvents, projects, settings };

export const db = drizzle(queryClient, { schema });
