import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";
import { messages } from "./schema/messages.js";
import { obligations } from "./schema/obligations.js";
import { contacts } from "./schema/contacts.js";
import { diary } from "./schema/diary.js";
import { memory } from "./schema/memory.js";
import { briefings } from "./schema/briefings.js";

const connectionString = process.env.DATABASE_URL;

if (!connectionString) {
  throw new Error("DATABASE_URL environment variable is required");
}

const queryClient = postgres(connectionString);

const schema = { messages, obligations, contacts, diary, memory, briefings };

export const db = drizzle(queryClient, { schema });
