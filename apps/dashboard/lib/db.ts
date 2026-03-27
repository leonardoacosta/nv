/**
 * Re-export the Drizzle database client from @nova/db.
 *
 * All API route handlers that need direct DB access should import from here.
 * The DATABASE_URL env var must be set (passed via docker-compose.yml).
 */
export { db } from "@nova/db";
