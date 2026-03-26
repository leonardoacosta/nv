import { NextResponse, type NextRequest } from "next/server";

const AUTH_COOKIE_NAME = "dashboard_token";

/**
 * Constant-time comparison using Web Crypto (Edge Runtime compatible).
 * Falls back to byte-by-byte with constant-time accumulation.
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

function getTokenFromEnv(): string | undefined {
  return process.env.DASHBOARD_TOKEN;
}

function isAuthEnabled(): boolean {
  const token = getTokenFromEnv();
  return typeof token === "string" && token.length > 0;
}

function verifyTokenValue(candidate: string): boolean {
  const token = getTokenFromEnv();
  if (!token) return false;
  return timingSafeCompare(candidate, token);
}

function addCorsHeaders(
  response: NextResponse,
  request: NextRequest,
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

export function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;

  // Handle CORS preflight
  if (request.method === "OPTIONS") {
    const response = new NextResponse(null, { status: 204 });
    return addCorsHeaders(response, request);
  }

  // Dev-mode: no auth when DASHBOARD_TOKEN is unset
  if (!isAuthEnabled()) {
    const response = NextResponse.next();
    return addCorsHeaders(response, request);
  }

  // WebSocket upgrade requests: check token query param
  if (pathname.startsWith("/ws/")) {
    const token = request.nextUrl.searchParams.get("token");
    if (!token || !verifyTokenValue(token)) {
      return new NextResponse(JSON.stringify({ error: "Unauthorized" }), {
        status: 401,
        headers: { "Content-Type": "application/json" },
      });
    }
    const response = NextResponse.next();
    return addCorsHeaders(response, request);
  }

  // API requests: check Authorization header
  if (pathname.startsWith("/api/")) {
    // Allow auth endpoints without token
    if (
      pathname === "/api/auth/verify" ||
      pathname === "/api/auth/logout"
    ) {
      const response = NextResponse.next();
      return addCorsHeaders(response, request);
    }

    const authHeader = request.headers.get("authorization");
    const token = authHeader?.startsWith("Bearer ")
      ? authHeader.slice(7)
      : null;

    if (!token || !verifyTokenValue(token)) {
      const response = new NextResponse(
        JSON.stringify({ error: "Unauthorized" }),
        { status: 401, headers: { "Content-Type": "application/json" } },
      );
      return addCorsHeaders(response, request);
    }

    const response = NextResponse.next();
    return addCorsHeaders(response, request);
  }

  // Page requests: check cookie
  const cookieToken = request.cookies.get(AUTH_COOKIE_NAME)?.value;
  if (!cookieToken || !verifyTokenValue(cookieToken)) {
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
