import { NextResponse, type NextRequest } from "next/server";
import { getSessionCookie } from "better-auth/cookies";

/**
 * Constant-time comparison using Web Crypto (Edge Runtime compatible).
 * Used for legacy DASHBOARD_TOKEN fallback during migration period.
 */
function timingSafeCompare(a: string, b: string): boolean {
  const encA = new TextEncoder().encode(a);
  const encB = new TextEncoder().encode(b);

  if (encA.length !== encB.length) {
    // Still iterate to avoid timing leak on length
    let result = 1;
    for (let i = 0; i < encB.length; i++) {
      result |= encB[i]! ^ encB[i]!;
    }
    return result === 0; // always false
  }

  let result = 0;
  for (let i = 0; i < encA.length; i++) {
    result |= encA[i]! ^ encB[i]!;
  }
  return result === 0;
}

// ---------------------------------------------------------------------------
// Auth helpers
// ---------------------------------------------------------------------------

/** Auth is enabled when BETTER_AUTH_SECRET is set (production) or DASHBOARD_TOKEN is set (legacy). */
function isAuthEnabled(): boolean {
  const secret = process.env.BETTER_AUTH_SECRET;
  const legacyToken = process.env.DASHBOARD_TOKEN;
  return (
    (typeof secret === "string" && secret.length > 0) ||
    (typeof legacyToken === "string" && legacyToken.length > 0)
  );
}

/** Check if the request has a valid Better Auth session cookie. */
function hasBetterAuthSession(request: NextRequest): boolean {
  const token = getSessionCookie(request);
  return token !== null;
}

/** Check if the request has a valid legacy DASHBOARD_TOKEN cookie. */
function hasLegacySession(request: NextRequest): boolean {
  const legacyToken = process.env.DASHBOARD_TOKEN;
  if (!legacyToken) return false;
  const cookieValue = request.cookies.get("dashboard_token")?.value;
  if (!cookieValue) return false;
  return timingSafeCompare(cookieValue, legacyToken);
}

/** Check if the request has a valid legacy Bearer token in Authorization header. */
function hasLegacyBearerToken(request: NextRequest): boolean {
  const legacyToken = process.env.DASHBOARD_TOKEN;
  if (!legacyToken) return false;
  const authHeader = request.headers.get("authorization");
  const token = authHeader?.startsWith("Bearer ") ? authHeader.slice(7) : null;
  if (!token) return false;
  return timingSafeCompare(token, legacyToken);
}

/** Returns true if the request is authenticated via either Better Auth or legacy token. */
function isAuthenticated(request: NextRequest): boolean {
  return (
    hasBetterAuthSession(request) ||
    hasLegacySession(request) ||
    hasLegacyBearerToken(request)
  );
}

// ---------------------------------------------------------------------------
// CORS
// ---------------------------------------------------------------------------

function addCorsHeaders(
  response: NextResponse,
  _request: NextRequest,
): NextResponse {
  const corsOrigin = process.env.DASHBOARD_CORS_ORIGIN;
  if (corsOrigin) {
    response.headers.set("Access-Control-Allow-Origin", corsOrigin);
    response.headers.set(
      "Access-Control-Allow-Methods",
      "GET, POST, PUT, DELETE, OPTIONS",
    );
    response.headers.set(
      "Access-Control-Allow-Headers",
      "Content-Type, Authorization",
    );
  }
  return response;
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

export function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;

  // Handle CORS preflight
  if (request.method === "OPTIONS") {
    const response = new NextResponse(null, { status: 204 });
    return addCorsHeaders(response, request);
  }

  // Dev-mode: no auth when neither BETTER_AUTH_SECRET nor DASHBOARD_TOKEN is set
  if (!isAuthEnabled()) {
    const response = NextResponse.next();
    return addCorsHeaders(response, request);
  }

  // Pass through Better Auth API routes (sign-in, sign-up, sign-out, etc.)
  if (pathname.startsWith("/api/auth/")) {
    const response = NextResponse.next();
    return addCorsHeaders(response, request);
  }

  // WebSocket upgrade requests: session cookie authenticates automatically
  if (pathname.startsWith("/ws/")) {
    if (!isAuthenticated(request)) {
      return new NextResponse(JSON.stringify({ error: "Unauthorized" }), {
        status: 401,
        headers: { "Content-Type": "application/json" },
      });
    }
    const response = NextResponse.next();
    return addCorsHeaders(response, request);
  }

  // API requests: check session cookie or Bearer token
  if (pathname.startsWith("/api/")) {
    // Allow tRPC auth procedures without authentication
    if (pathname.startsWith("/api/trpc/auth.")) {
      const response = NextResponse.next();
      return addCorsHeaders(response, request);
    }

    if (!isAuthenticated(request)) {
      const response = new NextResponse(
        JSON.stringify({ error: "Unauthorized" }),
        { status: 401, headers: { "Content-Type": "application/json" } },
      );
      return addCorsHeaders(response, request);
    }

    const response = NextResponse.next();
    return addCorsHeaders(response, request);
  }

  // Page requests: check session cookie (Better Auth or legacy)
  if (!hasBetterAuthSession(request) && !hasLegacySession(request)) {
    const loginUrl = request.nextUrl.clone();
    loginUrl.pathname = "/login";
    return NextResponse.redirect(loginUrl);
  }

  const response = NextResponse.next();
  return addCorsHeaders(response, request);
}

export const config = {
  matcher: [
    /*
     * Match all request paths except:
     * - /login (auth page itself)
     * - /_next/static (static files)
     * - /_next/image (image optimization)
     * - /favicon.ico
     */
    "/((?!login|_next/static|_next/image|favicon\\.ico).*)",
  ],
};
