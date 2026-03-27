import type { DigestSchedulerDeps } from "../../features/digest/index.js";
import { runTier1Digest, runTier2Digest } from "../../features/digest/index.js";

// Module-level deps reference — set once via setDigestDeps()
let _deps: DigestSchedulerDeps | null = null;

/**
 * Initialize the digest command with scheduler deps.
 * Must be called once at startup before any /digest commands.
 */
export function setDigestDeps(deps: DigestSchedulerDeps): void {
  _deps = deps;
}

/**
 * /digest — trigger immediate Tier 1 thin digest
 * /digest weekly — trigger immediate Tier 2 weekly LLM digest
 */
export async function buildDigestReply(subcommand?: string): Promise<string> {
  if (!_deps) {
    return "Digest system not initialized.";
  }

  if (subcommand === "weekly") {
    try {
      await runTier2Digest(_deps);
      return "Weekly digest triggered. Check above for results.";
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      return `Weekly digest failed: ${msg}`;
    }
  }

  // Default: Tier 1 thin digest
  try {
    await runTier1Digest(_deps);
    return "Digest triggered. Check above for results.";
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return `Digest failed: ${msg}`;
  }
}
