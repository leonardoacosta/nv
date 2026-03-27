/**
 * Server-side tRPC caller for RSC prefetch.
 *
 * Creates a caller with the auth token from the dashboard_token cookie.
 * Usage in RSC:
 *
 *   import { createServerCaller } from "@/lib/trpc/server";
 *   const trpc = await createServerCaller();
 *   const data = await trpc.obligation.list({ status: "open" });
 */

import { cookies } from "next/headers";
import { createCaller, type TRPCContext } from "@nova/api";

const AUTH_COOKIE_NAME = "dashboard_token";

/**
 * Create a server-side tRPC caller with auth from cookies.
 */
export async function createServerCaller() {
  const cookieStore = await cookies();
  const token = cookieStore.get(AUTH_COOKIE_NAME)?.value ?? null;

  const ctx: TRPCContext = { token };
  return createCaller(ctx);
}
