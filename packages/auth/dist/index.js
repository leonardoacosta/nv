import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { bearer } from "better-auth/plugins";
import { apiKey } from "@better-auth/api-key";
import { db, user, authSession, account, verification, apikey, } from "@nova/db";
// ---------------------------------------------------------------------------
// Better Auth server instance
//
// Env vars consumed:
//   BETTER_AUTH_SECRET  – session encryption secret (required in production)
//   BETTER_AUTH_URL     – dashboard base URL (required in production)
//
// The drizzle adapter receives the existing @nova/db instance and schema
// tables so Better Auth operates on the same Postgres connection.
// ---------------------------------------------------------------------------
export const auth = betterAuth({
    database: drizzleAdapter(db, {
        provider: "pg",
        schema: {
            user,
            session: authSession,
            account,
            verification,
            apikey,
        },
    }),
    emailAndPassword: {
        enabled: true,
    },
    plugins: [
        bearer(),
        apiKey(),
    ],
});
//# sourceMappingURL=index.js.map