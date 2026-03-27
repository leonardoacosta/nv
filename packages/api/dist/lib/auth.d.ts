/**
 * Standalone timing-safe token verification for the API package.
 *
 * Replicates the logic from apps/dashboard/lib/auth.ts without
 * creating a cross-dependency. Both implementations are <15 lines
 * and share the same constant-time comparison approach.
 */
export declare const AUTH_COOKIE_NAME = "dashboard_token";
/** Read the configured token from the environment. */
export declare function getToken(): string | undefined;
/** Returns true if DASHBOARD_TOKEN is set and non-empty. */
export declare function isAuthEnabled(): boolean;
/**
 * Constant-time comparison of a candidate token against the stored token.
 * Returns false if auth is disabled (no DASHBOARD_TOKEN set).
 */
export declare function verifyToken(candidate: string): boolean;
//# sourceMappingURL=auth.d.ts.map