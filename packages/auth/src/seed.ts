import { auth } from "./index.js";

// ---------------------------------------------------------------------------
// Seed script: create admin user + API key
//
// Idempotent -- skips user creation if email already exists.
// Prints the API key to stdout for storage in Doppler.
//
// Env vars:
//   NOVA_ADMIN_EMAIL    – admin email (default: leo@nova.local)
//   NOVA_ADMIN_PASSWORD – admin password (required)
//   BETTER_AUTH_SECRET  – required by auth instance
//   BETTER_AUTH_URL     – required by auth instance
//   DATABASE_URL        – required by @nova/db
//
// Usage:
//   pnpm --filter @nova/auth seed
// ---------------------------------------------------------------------------

async function seed() {
  const email = process.env.NOVA_ADMIN_EMAIL ?? "leo@nova.local";
  const password = process.env.NOVA_ADMIN_PASSWORD;

  if (!password) {
    console.error("NOVA_ADMIN_PASSWORD environment variable is required");
    process.exit(1);
  }

  // -------------------------------------------------------------------------
  // 1. Create admin user (idempotent)
  // -------------------------------------------------------------------------
  console.log(`Seeding admin user: ${email}`);

  // Try sign-up first. If user already exists, Better Auth returns an error
  // and we fall through to sign-in.
  let sessionToken: string | undefined;
  let userId: string | undefined;

  try {
    const signUpResult = await auth.api.signUpEmail({
      body: { name: "Admin", email, password },
      asResponse: false,
    });

    if (signUpResult?.user) {
      console.log(`Admin user created (id: ${signUpResult.user.id})`);
      userId = signUpResult.user.id;
      sessionToken = signUpResult.token ?? undefined;
    }
  } catch {
    // User likely already exists -- continue to sign-in
  }

  if (!userId) {
    // User already exists -- sign in to get a session
    const signInResult = await auth.api.signInEmail({
      body: { email, password },
      asResponse: false,
    });

    if (!signInResult?.user) {
      console.error("Failed to sign in as admin user");
      process.exit(1);
    }

    console.log(`Admin user already exists (id: ${signInResult.user.id})`);
    userId = signInResult.user.id;
    sessionToken = signInResult.token ?? undefined;
  }

  if (!sessionToken || !userId) {
    console.error("Failed to obtain session token");
    process.exit(1);
  }

  // -------------------------------------------------------------------------
  // 2. Generate API key using the authenticated session
  // -------------------------------------------------------------------------
  console.log("Generating API key...");

  try {
    const keyResult = await auth.api.createApiKey({
      body: { name: "nova-daemon" },
      headers: new Headers({
        Authorization: `Bearer ${sessionToken}`,
      }),
      asResponse: false,
    });

    if (keyResult && typeof keyResult === "object" && "key" in keyResult) {
      const key = (keyResult as { key: string }).key;
      console.log("\nAPI Key (store in Doppler as NOVA_DASHBOARD_API_KEY):");
      console.log(key);
    } else {
      console.log("\nUser created but API key generation returned unexpected result.");
      console.log("Result:", JSON.stringify(keyResult));
    }
  } catch (err) {
    console.log("\nUser seeded, but API key generation failed:");
    console.log(err instanceof Error ? err.message : String(err));
    console.log("Generate an API key manually via the dashboard or Better Auth API.");
  }
}

seed().catch((err) => {
  console.error("Seed failed:", err);
  process.exit(1);
});
