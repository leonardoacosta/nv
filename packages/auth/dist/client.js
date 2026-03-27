import { createAuthClient } from "better-auth/react";
import { apiKeyClient } from "@better-auth/api-key/client";
// ---------------------------------------------------------------------------
// Better Auth client for React (browser-side)
//
// The client auto-discovers the auth API at /api/auth (default basePath).
// Plugins must mirror the server-side plugin list.
// ---------------------------------------------------------------------------
export const authClient = createAuthClient({
    plugins: [apiKeyClient()],
});
// Convenience re-exports for common operations
export const { signIn, signUp, signOut, useSession, } = authClient;
//# sourceMappingURL=client.js.map