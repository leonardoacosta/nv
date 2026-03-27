import { type NextRequest, NextResponse } from "next/server";
import type { AutomationWatcher } from "@/types/api";

/**
 * In-memory watcher config override.
 *
 * The watcher config is sourced from environment variables at startup. This
 * module holds runtime overrides applied via PATCH. Values revert to env-var
 * defaults on process restart (v1 behaviour — a future spec can persist these
 * to a DB settings table).
 */
const watcherOverrides: Partial<{
  enabled: boolean;
  interval_minutes: number;
  quiet_start: string;
  quiet_end: string;
}> = {};

/** Read the current watcher state, merging env defaults with in-memory overrides. */
export function getWatcherState(): AutomationWatcher {
  return {
    enabled:
      watcherOverrides.enabled ??
      (process.env.WATCHER_ENABLED !== "false"),
    interval_minutes:
      watcherOverrides.interval_minutes ??
      parseInt(process.env.WATCHER_INTERVAL_MINUTES ?? "30", 10),
    quiet_start:
      watcherOverrides.quiet_start ??
      (process.env.WATCHER_QUIET_START ?? "22:00"),
    quiet_end:
      watcherOverrides.quiet_end ??
      (process.env.WATCHER_QUIET_END ?? "07:00"),
    last_run_at: null,
  };
}

// HH:MM validation (24-hour format)
const HH_MM_RE = /^([01]\d|2[0-3]):([0-5]\d)$/;

export async function PATCH(request: NextRequest) {
  try {
    const body: unknown = await request.json();

    if (typeof body !== "object" || body === null || Array.isArray(body)) {
      return NextResponse.json(
        { error: "Request body must be a JSON object." },
        { status: 400 },
      );
    }

    const patch = body as Record<string, unknown>;

    // Validate and apply each optional field
    if ("enabled" in patch) {
      if (typeof patch.enabled !== "boolean") {
        return NextResponse.json(
          { error: "'enabled' must be a boolean." },
          { status: 400 },
        );
      }
      watcherOverrides.enabled = patch.enabled;
    }

    if ("interval_minutes" in patch) {
      if (
        typeof patch.interval_minutes !== "number" ||
        !Number.isInteger(patch.interval_minutes) ||
        patch.interval_minutes < 5 ||
        patch.interval_minutes > 120
      ) {
        return NextResponse.json(
          { error: "'interval_minutes' must be an integer between 5 and 120." },
          { status: 400 },
        );
      }
      watcherOverrides.interval_minutes = patch.interval_minutes;
    }

    if ("quiet_start" in patch) {
      if (typeof patch.quiet_start !== "string" || !HH_MM_RE.test(patch.quiet_start)) {
        return NextResponse.json(
          { error: "'quiet_start' must be a string in HH:MM (24-hour) format." },
          { status: 400 },
        );
      }
      watcherOverrides.quiet_start = patch.quiet_start;
    }

    if ("quiet_end" in patch) {
      if (typeof patch.quiet_end !== "string" || !HH_MM_RE.test(patch.quiet_end)) {
        return NextResponse.json(
          { error: "'quiet_end' must be a string in HH:MM (24-hour) format." },
          { status: 400 },
        );
      }
      watcherOverrides.quiet_end = patch.quiet_end;
    }

    // Require at least one field
    const known = ["enabled", "interval_minutes", "quiet_start", "quiet_end"];
    const hasKnown = known.some((k) => k in patch);
    if (!hasKnown) {
      return NextResponse.json(
        {
          error:
            "Request body must include at least one of: enabled, interval_minutes, quiet_start, quiet_end.",
        },
        { status: 400 },
      );
    }

    return NextResponse.json(getWatcherState());
  } catch (e) {
    const message = e instanceof Error ? e.message : "Unknown error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
