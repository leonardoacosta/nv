/**
 * Dashboard authentication — re-exports from @nova/auth.
 *
 * Server-side code should import from here or directly from "@nova/auth".
 * Client-side code should import from "@nova/auth/client".
 */

export { auth } from "@nova/auth";
export type { Auth, Session, User } from "@nova/auth";
